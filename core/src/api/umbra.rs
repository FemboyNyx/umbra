//! Public API exposed to Dart via flutter_rust_bridge v2.
//!
//! All functions operate on a single process-wide core instance created by
//! [`init`]. Async functions run on core's own tokio runtime, so they work no
//! matter which executor frb uses. See `docs/core-api.md`.

use std::sync::{Arc, LazyLock, Mutex};

use anyhow::{anyhow, bail, Result};

pub use crate::api::types::*;
use crate::engine::core::Core;
use crate::engine::events::EventHub;
use crate::frb_generated::StreamSink;
use crate::mock::MockCore;

#[derive(Clone)]
pub(crate) enum Backend {
    Mock(Arc<MockCore>),
    Real(Arc<Core>),
}

struct Global {
    data_dir: String,
    backend: Backend,
}

static RUNTIME: LazyLock<tokio::runtime::Runtime> = LazyLock::new(|| {
    tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .thread_name("umbra-core")
        .enable_all()
        .build()
        .expect("failed to start umbra-core runtime")
});
static HUB: LazyLock<EventHub> = LazyLock::new(EventHub::new);
static GLOBAL: Mutex<Option<Global>> = Mutex::new(None);

fn backend() -> Result<Backend> {
    let g = GLOBAL.lock().map_err(|_| anyhow!("core state poisoned"))?;
    let g = g
        .as_ref()
        .ok_or_else(|| anyhow!("core not initialized; call init() first"))?;
    Ok(g.backend.clone())
}

async fn on_runtime<T: Send + 'static>(
    fut: impl std::future::Future<Output = Result<T>> + Send + 'static,
) -> Result<T> {
    RUNTIME
        .spawn(fut)
        .await
        .map_err(|e| anyhow!("core task failed: {e}"))?
}

/// Open (or create) the encrypted store in `data_dir` and load identity.
///
/// `db_key` must be exactly 32 random bytes kept in the OS keystore by the
/// app; core never persists it. Calling `init` again with the same `data_dir`
/// (e.g. after a Flutter hot restart) returns the current [`SelfInfo`].
pub async fn init(data_dir: String, db_key: Vec<u8>, cfg: CoreConfig) -> Result<SelfInfo> {
    let existing = {
        let g = GLOBAL.lock().map_err(|_| anyhow!("core state poisoned"))?;
        g.as_ref().map(|g| g.data_dir == data_dir)
    };
    match existing {
        Some(true) => return self_info(),
        Some(false) => bail!("core already initialized with a different data_dir"),
        None => {}
    }
    if !cfg.mock && db_key.len() != 32 {
        bail!("db_key must be 32 bytes");
    }
    let backend = if cfg.mock {
        Backend::Mock(MockCore::new(HUB.clone(), RUNTIME.handle().clone()))
    } else {
        let dir = std::path::PathBuf::from(&data_dir);
        let key = zeroize::Zeroizing::new(db_key);
        let hub = HUB.clone();
        let core = on_runtime(async move { Core::open(&dir, &key, cfg, hub) }).await?;
        Backend::Real(core)
    };
    let mut g = GLOBAL.lock().map_err(|_| anyhow!("core state poisoned"))?;
    if g.is_none() {
        *g = Some(Global { data_dir, backend });
    }
    drop(g);
    self_info()
}

/// Start libp2p (listen, mDNS/DHT/relay per config) and the outbox retry loop.
pub async fn start_network() -> Result<()> {
    match backend()? {
        Backend::Mock(m) => m.start_network(),
        Backend::Real(c) => on_runtime(async move { c.start_network().await }).await,
    }
}

pub async fn stop_network() -> Result<()> {
    match backend()? {
        Backend::Mock(m) => m.stop_network(),
        Backend::Real(c) => on_runtime(async move { c.stop_network().await }).await,
    }
}

/// Subscribe to the single [`CoreEvent`] stream. Calling it again replaces
/// the previous subscription (the old Dart stream stops receiving events).
pub fn events(sink: StreamSink<CoreEvent>) -> Result<()> {
    let mut rx = HUB.subscribe_exclusive();
    RUNTIME.spawn(async move {
        while let Some(ev) = rx.recv().await {
            if sink.add(ev).is_err() {
                break;
            }
        }
    });
    Ok(())
}

/// Current network state (also pushed as `CoreEvent::NetworkStatus`).
pub fn network_status() -> Result<NetworkInfo> {
    Ok(match backend()? {
        Backend::Mock(m) => m.network_status(),
        Backend::Real(c) => c.network_info(),
    })
}

pub fn self_info() -> Result<SelfInfo> {
    Ok(match backend()? {
        Backend::Mock(m) => m.self_info(),
        Backend::Real(c) => c.self_info(),
    })
}

pub fn set_display_name(name: String) -> Result<()> {
    match backend()? {
        Backend::Mock(m) => m.set_display_name(name),
        Backend::Real(c) => c.set_display_name(name),
    }
}

/// Parse and validate an `umbra://add?...` QR payload. Works before `init`.
pub fn parse_qr(payload: String) -> Result<ContactCard> {
    crate::engine::qr::parse_qr(&payload)
}

pub async fn add_contact(card: ContactCard, verified: bool) -> Result<Contact> {
    match backend()? {
        Backend::Mock(m) => on_runtime(async move { m.add_contact(card, verified) }).await,
        Backend::Real(c) => on_runtime(async move { c.add_contact(card, verified).await }).await,
    }
}

pub fn list_contacts() -> Result<Vec<Contact>> {
    match backend()? {
        Backend::Mock(m) => Ok(m.list_contacts()),
        Backend::Real(c) => c.list_contacts(),
    }
}

pub fn set_verified(peer_id: String, verified: bool) -> Result<()> {
    match backend()? {
        Backend::Mock(m) => m.set_verified(&peer_id, verified),
        Backend::Real(c) => c.set_verified(&peer_id, verified),
    }
}

/// Remove the contact, its Olm sessions, messages and pending outbox.
pub fn remove_contact(peer_id: String) -> Result<()> {
    match backend()? {
        Backend::Mock(m) => m.remove_contact(&peer_id),
        Backend::Real(c) => c.remove_contact(&peer_id),
    }
}

/// Queue an E2E-encrypted text message. Returns immediately with the id;
/// progress arrives as `CoreEvent::MessageStatus`.
pub async fn send_text(peer_id: String, body: String) -> Result<MessageId> {
    match backend()? {
        Backend::Mock(m) => {
            on_runtime(async move {
                let id = m.send_text(peer_id.clone(), body.clone())?;
                tokio::spawn(m.clone().simulate_reply(peer_id, id.clone(), body));
                Ok(id)
            })
            .await
        }
        Backend::Real(c) => on_runtime(async move { c.send_text(&peer_id, body).await }).await,
    }
}

/// Conversations, most recent first.
pub fn list_conversations() -> Result<Vec<ConversationSummary>> {
    match backend()? {
        Backend::Mock(m) => Ok(m.list_conversations()),
        Backend::Real(c) => c.list_conversations(),
    }
}

/// Messages with `peer_id`, newest first. `before` is an exclusive
/// `timestamp_ms` cursor for paging.
pub fn list_messages(peer_id: String, before: Option<i64>, limit: u32) -> Result<Vec<Message>> {
    match backend()? {
        Backend::Mock(m) => Ok(m.list_messages(&peer_id, before, limit)),
        Backend::Real(c) => c.list_messages(&peer_id, before, limit),
    }
}

/// Mark incoming messages from `peer_id` up to and including `up_to` as read.
pub fn mark_read(peer_id: String, up_to: MessageId) -> Result<()> {
    match backend()? {
        Backend::Mock(m) => m.mark_read(&peer_id, &up_to),
        Backend::Real(c) => c.mark_read(&peer_id, &up_to),
    }
}

/// Send an encrypted call-signaling message (not stored, not queued).
/// Fails if the peer cannot be reached right now.
pub async fn send_signal(
    peer_id: String,
    call_id: String,
    kind: SignalKind,
    payload: String,
) -> Result<()> {
    match backend()? {
        Backend::Mock(m) => m.send_signal(peer_id, call_id, kind, payload),
        Backend::Real(c) => {
            on_runtime(async move { c.send_signal(&peer_id, call_id, kind, payload).await }).await
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_subscribe() -> tokio::sync::mpsc::UnboundedReceiver<CoreEvent> {
        HUB.subscribe()
    }

    #[test]
    fn mock_round_trip() {
        let mut rx = test_subscribe();
        let me = RUNTIME
            .block_on(init(
                "mock".into(),
                vec![],
                CoreConfig {
                    mock: true,
                    ..Default::default()
                },
            ))
            .unwrap();
        assert!(me.qr_payload.starts_with("umbra://add?"));
        let card = parse_qr(me.qr_payload.clone()).unwrap();
        assert_eq!(card.peer_id, me.peer_id);
        RUNTIME.block_on(start_network()).unwrap();
        // Wait for the fake contact request, accept it, chat, call.
        let bot = RUNTIME.block_on(async {
            loop {
                if let Some(CoreEvent::ContactRequest { card }) = rx.recv().await {
                    break card;
                }
            }
        });
        let c = RUNTIME.block_on(add_contact(bot, false)).unwrap();
        assert_eq!(c.safety_number.split(' ').count(), 12);
        let id = RUNTIME
            .block_on(send_text(c.peer_id.clone(), "hi".into()))
            .unwrap();
        RUNTIME
            .block_on(send_signal(
                c.peer_id.clone(),
                "call1".into(),
                SignalKind::Offer,
                "sdp".into(),
            ))
            .unwrap();
        let (mut delivered, mut echo, mut answer) = (false, false, false);
        RUNTIME.block_on(async {
            while !(delivered && echo && answer) {
                match tokio::time::timeout(std::time::Duration::from_secs(5), rx.recv())
                    .await
                    .unwrap()
                    .unwrap()
                {
                    CoreEvent::MessageStatus {
                        id: i,
                        status: DeliveryStatus::Delivered,
                        ..
                    } if i == id => delivered = true,
                    CoreEvent::MessageReceived { message } if message.body == "echo: hi" => {
                        echo = true
                    }
                    CoreEvent::Signal {
                        kind: SignalKind::Answer,
                        ..
                    } => answer = true,
                    _ => {}
                }
            }
        });
        let msgs = list_messages(c.peer_id.clone(), None, 10).unwrap();
        assert_eq!(msgs.len(), 2);
        assert_eq!(list_conversations().unwrap()[0].unread_count, 1);
        mark_read(c.peer_id.clone(), msgs[0].id.clone()).unwrap();
        assert_eq!(list_conversations().unwrap()[0].unread_count, 0);
    }
}
