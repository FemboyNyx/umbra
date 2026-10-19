//! In-memory mock backend (`CoreConfig { mock: true, .. }`).
//!
//! No disk, no network. Behaviour, so the UI can be exercised end to end:
//! * `start_network` emits a running `NetworkStatus`.
//! * ~2 s after `init`, a `ContactRequest` arrives from "Mock Bot".
//! * `add_contact` emits `ContactOnline` after 300 ms.
//! * `send_text` emits `Sent`, then `Delivered` (400 ms), then an echo reply
//!   `MessageReceived` (1 s).
//! * `send_signal(Offer)` answers with `Ringing` then `Answer` (payload
//!   `"mock-answer-sdp"`); `IceCandidate` is ignored; `Hangup` is echoed back.

use std::sync::Mutex;
use std::time::Duration;

use anyhow::{anyhow, Result};
use libp2p::identity::Keypair;

use crate::api::types::*;
use crate::engine::events::EventHub;
use crate::engine::util::{b64, now_ms};
use crate::engine::{qr, safety};

struct MockState {
    self_info: SelfInfo,
    fp_digits: String,
    contacts: Vec<Contact>,
    messages: Vec<Message>,
    network: NetworkInfo,
}

pub struct MockCore {
    st: Mutex<MockState>,
    events: EventHub,
    rt: tokio::runtime::Handle,
}

fn fake_identity(name: &str) -> (ContactCard, String) {
    let kp = Keypair::generate_ed25519();
    let peer = kp.public().to_peer_id();
    let ed = kp
        .clone()
        .try_into_ed25519()
        .expect("ed25519")
        .public()
        .to_bytes();
    let ik = vodozemac::olm::Account::new().curve25519_key();
    let fp = safety::fingerprint_digits(&ed, ik.as_bytes(), &peer.to_bytes());
    (
        ContactCard {
            peer_id: peer.to_base58(),
            identity_key: ik.to_base64(),
            ed25519_key: b64(&ed),
            addrs: vec![format!("/ip4/192.0.2.1/tcp/4001/p2p/{peer}")],
            display_name: Some(name.to_string()),
        },
        fp,
    )
}

impl MockCore {
    pub fn new(events: EventHub, rt: tokio::runtime::Handle) -> std::sync::Arc<Self> {
        let (card, fp) = fake_identity("");
        let self_info = SelfInfo {
            peer_id: card.peer_id.clone(),
            fingerprint: safety::group(&fp),
            qr_payload: qr::build_qr(&card),
            display_name: String::new(),
            identity_key: card.identity_key,
            ed25519_key: card.ed25519_key,
            addrs: card.addrs,
        };
        let me = std::sync::Arc::new(Self {
            st: Mutex::new(MockState {
                self_info,
                fp_digits: fp,
                contacts: vec![],
                messages: vec![],
                network: NetworkInfo::default(),
            }),
            events,
            rt,
        });
        let (bot, _) = fake_identity("Mock Bot");
        me.later(2000, CoreEvent::ContactRequest { card: bot });
        me
    }

    fn later(&self, ms: u64, ev: CoreEvent) {
        let events = self.events.clone();
        self.rt.spawn(async move {
            tokio::time::sleep(Duration::from_millis(ms)).await;
            events.emit(ev);
        });
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, MockState> {
        self.st.lock().expect("mock poisoned")
    }

    pub fn self_info(&self) -> SelfInfo {
        self.lock().self_info.clone()
    }

    pub fn set_display_name(&self, name: String) -> Result<()> {
        let mut st = self.lock();
        st.self_info.display_name = name;
        Ok(())
    }

    pub fn start_network(&self) -> Result<()> {
        let info = {
            let mut st = self.lock();
            st.network = NetworkInfo {
                running: true,
                listen_addrs: vec![
                    "/ip4/192.0.2.10/tcp/4001".into(),
                    "/ip4/192.0.2.10/udp/4001/quic-v1".into(),
                ],
                connected_peers: st.contacts.len() as u32,
                relay_reservations: 0,
            };
            st.network.clone()
        };
        self.events.emit(CoreEvent::NetworkStatus { info });
        Ok(())
    }

    pub fn stop_network(&self) -> Result<()> {
        let info = {
            let mut st = self.lock();
            st.network = NetworkInfo::default();
            st.network.clone()
        };
        self.events.emit(CoreEvent::NetworkStatus { info });
        Ok(())
    }

    pub fn network_status(&self) -> NetworkInfo {
        self.lock().network.clone()
    }

    pub fn add_contact(&self, card: ContactCard, verified: bool) -> Result<Contact> {
        qr::validate_card(&card)?;
        let mut st = self.lock();
        let ed = crate::engine::util::unb64(&card.ed25519_key)?;
        let ik = vodozemac::Curve25519PublicKey::from_base64(&card.identity_key)?;
        let peer: libp2p::PeerId = card.peer_id.parse()?;
        let theirs = safety::fingerprint_digits(&ed, ik.as_bytes(), &peer.to_bytes());
        let c = Contact {
            peer_id: card.peer_id.clone(),
            display_name: card.display_name.clone().unwrap_or_default(),
            identity_key: card.identity_key,
            ed25519_key: card.ed25519_key,
            addrs: card.addrs,
            verified,
            safety_number: safety::safety_number(&st.fp_digits, &theirs),
            added_at_ms: now_ms(),
            online: true,
        };
        st.contacts.retain(|x| x.peer_id != c.peer_id);
        st.contacts.push(c.clone());
        drop(st);
        self.later(
            300,
            CoreEvent::ContactOnline {
                peer_id: c.peer_id.clone(),
            },
        );
        Ok(c)
    }

    pub fn list_contacts(&self) -> Vec<Contact> {
        self.lock().contacts.clone()
    }

    pub fn set_verified(&self, peer_id: &str, v: bool) -> Result<()> {
        let mut st = self.lock();
        let c = st
            .contacts
            .iter_mut()
            .find(|c| c.peer_id == peer_id)
            .ok_or_else(|| anyhow!("unknown contact"))?;
        c.verified = v;
        Ok(())
    }

    pub fn remove_contact(&self, peer_id: &str) -> Result<()> {
        let mut st = self.lock();
        st.contacts.retain(|c| c.peer_id != peer_id);
        st.messages.retain(|m| m.peer_id != peer_id);
        Ok(())
    }

    fn require_contact(&self, peer_id: &str) -> Result<()> {
        if self.lock().contacts.iter().any(|c| c.peer_id == peer_id) {
            Ok(())
        } else {
            Err(anyhow!("unknown contact {peer_id}"))
        }
    }

    pub fn send_text(&self, peer_id: String, body: String) -> Result<MessageId> {
        self.require_contact(&peer_id)?;
        let id = uuid::Uuid::new_v4().to_string();
        let msg = Message {
            id: id.clone(),
            peer_id: peer_id.clone(),
            outgoing: true,
            body: body.clone(),
            timestamp_ms: now_ms(),
            status: DeliveryStatus::Sent,
            read: true,
        };
        self.lock().messages.push(msg);
        self.events.emit(CoreEvent::MessageStatus {
            id: id.clone(),
            peer_id: peer_id.clone(),
            status: DeliveryStatus::Sent,
        });

        Ok(id)
    }

    /// Called by the API layer (holding an `Arc<MockCore>`) to finish the
    /// simulated round trip of `send_text`.
    pub async fn simulate_reply(
        self: std::sync::Arc<Self>,
        peer_id: String,
        id: MessageId,
        body: String,
    ) {
        tokio::time::sleep(Duration::from_millis(400)).await;
        {
            let mut st = self.lock();
            if let Some(m) = st.messages.iter_mut().find(|m| m.id == id) {
                m.status = DeliveryStatus::Delivered;
            }
        }
        self.events.emit(CoreEvent::MessageStatus {
            id,
            peer_id: peer_id.clone(),
            status: DeliveryStatus::Delivered,
        });
        tokio::time::sleep(Duration::from_millis(600)).await;
        let reply = Message {
            id: uuid::Uuid::new_v4().to_string(),
            peer_id,
            outgoing: false,
            body: format!("echo: {body}"),
            timestamp_ms: now_ms(),
            status: DeliveryStatus::Received,
            read: false,
        };
        self.lock().messages.push(reply.clone());
        self.events
            .emit(CoreEvent::MessageReceived { message: reply });
    }

    pub fn list_conversations(&self) -> Vec<ConversationSummary> {
        let st = self.lock();
        let mut out: Vec<ConversationSummary> = st
            .contacts
            .iter()
            .map(|c| {
                let msgs: Vec<&Message> = st
                    .messages
                    .iter()
                    .filter(|m| m.peer_id == c.peer_id)
                    .collect();
                let last = msgs
                    .iter()
                    .max_by_key(|m| m.timestamp_ms)
                    .map(|m| (*m).clone());
                ConversationSummary {
                    peer_id: c.peer_id.clone(),
                    display_name: c.display_name.clone(),
                    unread_count: msgs.iter().filter(|m| !m.outgoing && !m.read).count() as u32,
                    updated_at_ms: last
                        .as_ref()
                        .map(|m| m.timestamp_ms)
                        .unwrap_or(c.added_at_ms),
                    last_message: last,
                }
            })
            .collect();
        out.sort_by_key(|c| std::cmp::Reverse(c.updated_at_ms));
        out
    }

    pub fn list_messages(&self, peer_id: &str, before: Option<i64>, limit: u32) -> Vec<Message> {
        let st = self.lock();
        let mut v: Vec<Message> = st
            .messages
            .iter()
            .filter(|m| m.peer_id == peer_id && before.is_none_or(|b| m.timestamp_ms < b))
            .cloned()
            .collect();
        v.sort_by_key(|m| std::cmp::Reverse(m.timestamp_ms));
        v.truncate(limit as usize);
        v
    }

    pub fn mark_read(&self, peer_id: &str, up_to: &str) -> Result<()> {
        let mut st = self.lock();
        let ts = st
            .messages
            .iter()
            .find(|m| m.id == up_to)
            .map(|m| m.timestamp_ms)
            .ok_or_else(|| anyhow!("unknown message"))?;
        for m in st
            .messages
            .iter_mut()
            .filter(|m| m.peer_id == peer_id && m.timestamp_ms <= ts)
        {
            m.read = true;
        }
        Ok(())
    }

    pub fn send_signal(
        &self,
        peer_id: String,
        call_id: String,
        kind: SignalKind,
        _payload: String,
    ) -> Result<()> {
        self.require_contact(&peer_id)?;
        match kind {
            SignalKind::Offer => {
                self.later(
                    300,
                    CoreEvent::Signal {
                        peer_id: peer_id.clone(),
                        call_id: call_id.clone(),
                        kind: SignalKind::Ringing,
                        payload: String::new(),
                    },
                );
                self.later(
                    1200,
                    CoreEvent::Signal {
                        peer_id,
                        call_id,
                        kind: SignalKind::Answer,
                        payload: "mock-answer-sdp".into(),
                    },
                );
            }
            SignalKind::Hangup => {
                self.later(
                    100,
                    CoreEvent::Signal {
                        peer_id,
                        call_id,
                        kind: SignalKind::Hangup,
                        payload: String::new(),
                    },
                );
            }
            _ => {}
        }
        Ok(())
    }
}
