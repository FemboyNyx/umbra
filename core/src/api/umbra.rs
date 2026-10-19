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

/// Trust a contact's new identity key after `CoreEvent::KeyChanged`.
/// Drops old Olm sessions, sets `verified = false` and clears the flag.
/// Only call this after the user re-compared safety numbers.
pub fn accept_key_change(peer_id: String) -> Result<Contact> {
    match backend()? {
        Backend::Mock(m) => m.accept_key_change(&peer_id),
        Backend::Real(c) => {
            let _rt = RUNTIME.enter();
            c.accept_key_change(&peer_id)
        }
    }
}

/// Keep refusing a contact's new identity key. The old key stays trusted;
/// the refused key is remembered and won't alert again (a different one will).
pub fn reject_key_change(peer_id: String) -> Result<()> {
    match backend()? {
        Backend::Mock(m) => m.reject_key_change(&peer_id),
        Backend::Real(c) => c.reject_key_change(&peer_id),
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

// ---- servers (docs/core-api.md#servers) ----

/// Run a server call on the real core's runtime. The mock backend has no
/// servers yet.
async fn server_call<T: Send + 'static>(
    f: impl FnOnce(Arc<Core>) -> Result<T> + Send + 'static,
) -> Result<T> {
    match backend()? {
        Backend::Mock(_) => bail!("servers are not available in mock mode yet"),
        Backend::Real(c) => on_runtime(async move { f(c) }).await,
    }
}

/// Create a server we own, with a `general` text channel. `recovery_keys`:
/// 1 to 3 Ed25519 public keys (base64); empty = generate one and return its
/// secret once in `CreatedServer.recovery_secret` (core never stores it).
/// `display_name`: our name in this server (`None` = the global one).
pub async fn create_server(
    name: String,
    recovery_keys: Vec<String>,
    display_name: Option<String>,
) -> Result<CreatedServer> {
    server_call(move |c| c.create_server(&name, recovery_keys, display_name)).await
}

pub async fn list_servers() -> Result<Vec<ServerInfo>> {
    server_call(|c| c.list_servers()).await
}

pub async fn server_info(server_id: String) -> Result<ServerInfo> {
    server_call(move |c| c.server_info(&server_id)).await
}

/// Leave and delete everything kept about the server.
pub async fn leave_server(server_id: String) -> Result<()> {
    server_call(move |c| c.leave_server(&server_id)).await
}

pub async fn list_channels(server_id: String) -> Result<Vec<ServerChannel>> {
    server_call(move |c| c.list_channels(&server_id)).await
}

/// Owner and admins.
pub async fn create_channel(server_id: String, name: String) -> Result<ServerChannel> {
    server_call(move |c| c.create_channel(&server_id, &name)).await
}

/// Owner and admins.
pub async fn rename_channel(server_id: String, channel_id: String, name: String) -> Result<()> {
    server_call(move |c| c.rename_channel(&server_id, &channel_id, &name)).await
}

pub async fn list_members(server_id: String) -> Result<Vec<ServerMember>> {
    server_call(move |c| c.list_members(&server_id)).await
}

/// Owner only: `Admin` or `Member` (no ownership transfer yet).
pub async fn set_role(server_id: String, peer_id: String, role: ServerRole) -> Result<()> {
    server_call(move |c| c.set_role(&server_id, &peer_id, role)).await
}

/// Owner only: roles that may create invites (the owner always may).
pub async fn set_invite_roles(server_id: String, roles: Vec<ServerRole>) -> Result<()> {
    server_call(move |c| c.set_invite_roles(&server_id, roles)).await
}

/// Owner only: whether new members see history from before they joined.
/// Default `true`.
pub async fn set_history_shared(server_id: String, shared: bool) -> Result<()> {
    server_call(move |c| c.set_history_shared(&server_id, shared)).await
}

/// Our display name in this server (`None` = the global one).
pub async fn set_server_display_name(
    server_id: String,
    display_name: Option<String>,
) -> Result<()> {
    server_call(move |c| c.set_server_display_name(&server_id, display_name)).await
}

/// Owner, or admins on members.
pub async fn kick(server_id: String, peer_id: String) -> Result<()> {
    server_call(move |c| c.kick(&server_id, &peer_id)).await
}

/// Owner, or admins on members.
pub async fn ban(server_id: String, peer_id: String) -> Result<()> {
    server_call(move |c| c.ban(&server_id, &peer_id)).await
}

pub async fn unban(server_id: String, peer_id: String) -> Result<()> {
    server_call(move |c| c.unban(&server_id, &peer_id)).await
}

/// Only roles in the server's invite roles (default owner and admin).
pub async fn create_invite(server_id: String, opts: InviteOptions) -> Result<CreatedInvite> {
    server_call(move |c| c.create_invite(&server_id, opts)).await
}

pub async fn revoke_invite(invite_id: String) -> Result<()> {
    server_call(move |c| c.revoke_invite(&invite_id)).await
}

/// Check an `umbra://join` link and describe it (show `owner_fingerprint`).
pub async fn parse_invite(link: String) -> Result<InvitePreview> {
    server_call(move |c| c.parse_invite(&link)).await
}

/// Join with an `umbra://join` link (network must be started). Returns with
/// status `Joining`; `ServerEvent::Joined` follows. Errors if refused.
pub async fn join_server(link: String, display_name: Option<String>) -> Result<ServerInfo> {
    match backend()? {
        Backend::Mock(_) => bail!("servers are not available in mock mode yet"),
        Backend::Real(c) => {
            on_runtime(async move { c.join_server(&link, display_name).await }).await
        }
    }
}

pub async fn join_requests(server_id: String) -> Result<Vec<JoinRequest>> {
    server_call(move |c| c.join_requests(&server_id)).await
}

/// Admit (`approve = true`) or drop a request for an approval invite.
pub async fn approve_join(server_id: String, peer_id: String, approve: bool) -> Result<()> {
    server_call(move |c| c.approve_join(&server_id, &peer_id, approve)).await
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
                SignalKind::Ringing,
                String::new(),
            ))
            .unwrap();
        RUNTIME
            .block_on(send_signal(
                c.peer_id.clone(),
                "call1".into(),
                SignalKind::Offer,
                "sdp".into(),
            ))
            .unwrap();
        let (mut delivered, mut echo, mut answer, mut accept) = (false, false, false, false);
        RUNTIME.block_on(async {
            while !(delivered && echo && answer && accept) {
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
                    CoreEvent::Signal {
                        kind: SignalKind::Accept,
                        ..
                    } => accept = true,
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
