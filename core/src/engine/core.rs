//! The core engine: ties identity, store, Olm and the network together.
//!
//! Usable directly from Rust (tests run several instances in one process);
//! the FFI layer in `crate::api` wraps a single global instance.

use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use anyhow::{anyhow, bail, Context, Result};
use libp2p::multiaddr::Protocol;
use libp2p::{Multiaddr, PeerId};
use vodozemac::Curve25519PublicKey;

use crate::api::types::*;
use crate::engine::crypto::{fingerprint_of, Crypto, Identity};
use crate::engine::events::EventHub;
use crate::engine::net::{self, NetConfig, NetHandle, TransportMode};
use crate::engine::proto::*;
use crate::engine::store::{ContactRow, Store};
use crate::engine::util::{now_ms, unb64};
use crate::engine::{qr, safety};

pub const DB_FILE: &str = "umbra.db";
const META_DISPLAY_NAME: &str = "display_name";
/// Undelivered messages older than this are marked `Failed`.
pub const OUTBOX_TTL_MS: i64 = 7 * 24 * 3600 * 1000;
const SIGNAL_TIMEOUT: Duration = Duration::from_secs(25);

#[derive(Default)]
struct NetState {
    handle: Option<NetHandle>,
    task: Option<tokio::task::JoinHandle<()>>,
    listen_addrs: Vec<Multiaddr>,
    connected: HashSet<PeerId>,
    /// Peers we've sent `Hello` to on the current connection.
    greeted: HashSet<PeerId>,
    connected_count: usize,
}

pub struct Core {
    id: Identity,
    store: Arc<Store>,
    crypto: Crypto,
    events: EventHub,
    cfg: CoreConfig,
    net: Mutex<NetState>,
    peer_locks: Mutex<HashMap<PeerId, Arc<tokio::sync::Mutex<()>>>>,
}

enum DeliverError {
    /// Recipient has not accepted us yet: stop and wait for their Hello.
    NotAContact,
    /// Transient (offline, timeout, ...): keep in the outbox.
    Retry(anyhow::Error),
}

impl From<anyhow::Error> for DeliverError {
    fn from(e: anyhow::Error) -> Self {
        DeliverError::Retry(e)
    }
}

fn parse_peer(s: &str) -> Result<PeerId> {
    s.parse().map_err(|_| anyhow!("invalid peer id {s}"))
}

fn parse_addrs(v: &[String]) -> Vec<Multiaddr> {
    v.iter().filter_map(|a| a.parse().ok()).collect()
}

fn is_circuit(a: &Multiaddr) -> bool {
    a.iter().any(|p| p == Protocol::P2pCircuit)
}

fn is_loopback_or_unspecified(a: &Multiaddr) -> bool {
    match a.iter().next() {
        Some(Protocol::Ip4(ip)) => ip.is_loopback() || ip.is_unspecified(),
        Some(Protocol::Ip6(ip)) => ip.is_loopback() || ip.is_unspecified(),
        _ => false,
    }
}

impl Core {
    /// Open `data_dir/umbra.db` with `db_key` and load (or create) identity.
    pub fn open(
        data_dir: &Path,
        db_key: &[u8],
        cfg: CoreConfig,
        events: EventHub,
    ) -> Result<Arc<Self>> {
        std::fs::create_dir_all(data_dir)
            .with_context(|| format!("create {}", data_dir.display()))?;
        let store = Arc::new(Store::open(&data_dir.join(DB_FILE), db_key)?);
        let (id, crypto) = Crypto::load_or_create(store.clone(), db_key)?;
        Ok(Arc::new(Self {
            id,
            store,
            crypto,
            events,
            cfg,
            net: Mutex::new(NetState::default()),
            peer_locks: Mutex::new(HashMap::new()),
        }))
    }

    fn ns(&self) -> MutexGuard<'_, NetState> {
        self.net.lock().expect("net state poisoned")
    }

    pub fn peer_id(&self) -> PeerId {
        self.id.peer_id
    }

    fn display_name(&self) -> String {
        self.store
            .meta_get(META_DISPLAY_NAME)
            .ok()
            .flatten()
            .and_then(|v| String::from_utf8(v).ok())
            .unwrap_or_default()
    }

    /// Tor-only when `tor_socks` is set or the crate is built with `tor`.
    pub fn tor_mode(&self) -> bool {
        self.cfg.tor_socks.is_some() || cfg!(feature = "tor")
    }

    /// Addresses we advertise (QR + Hello).
    fn advertised_addrs(&self) -> Vec<String> {
        let ns = self.ns();
        let tor = self.tor_mode();
        let mut out: Vec<&Multiaddr> = ns
            .listen_addrs
            .iter()
            .filter(|a| {
                if tor {
                    is_circuit(a)
                } else {
                    !is_loopback_or_unspecified(a) || is_circuit(a)
                }
            })
            .collect();
        if out.is_empty() && !tor {
            out = ns.listen_addrs.iter().filter(|a| !is_circuit(a)).collect();
        }
        // Prefer circuit addrs last so direct paths are tried first.
        out.sort_by_key(|a| is_circuit(a));
        out.into_iter()
            .take(qr::MAX_QR_ADDRS)
            .map(|a| a.to_string())
            .collect()
    }

    pub fn self_card(&self) -> ContactCard {
        let name = self.display_name();
        ContactCard {
            peer_id: self.id.peer_id.to_base58(),
            identity_key: self.id.curve25519_b64(),
            ed25519_key: self.id.ed25519_b64(),
            addrs: self.advertised_addrs(),
            display_name: (!name.is_empty()).then_some(name),
        }
    }

    pub fn self_info(&self) -> SelfInfo {
        let card = self.self_card();
        SelfInfo {
            peer_id: card.peer_id.clone(),
            fingerprint: safety::group(&self.id.fp_digits),
            qr_payload: qr::build_qr(&card),
            display_name: card.display_name.clone().unwrap_or_default(),
            identity_key: card.identity_key,
            ed25519_key: card.ed25519_key,
            addrs: card.addrs,
        }
    }

    pub fn set_display_name(&self, name: String) -> Result<()> {
        let name = name.trim().to_string();
        if name.len() > 64 {
            bail!("display name too long (max 64 bytes)");
        }
        self.store.meta_set(META_DISPLAY_NAME, name.as_bytes())
    }

    fn safety_for(&self, c: &ContactRow) -> String {
        let theirs = (|| -> Result<String> {
            let peer = parse_peer(&c.peer_id)?;
            let ed = unb64(&c.ed25519_key)?;
            let ik = Curve25519PublicKey::from_base64(&c.identity_key)?;
            Ok(fingerprint_of(&peer, &ed, &ik))
        })()
        .unwrap_or_default();
        safety::safety_number(&self.id.fp_digits, &theirs)
    }

    fn to_contact(&self, row: ContactRow) -> Contact {
        let online = parse_peer(&row.peer_id)
            .map(|p| self.ns().connected.contains(&p))
            .unwrap_or(false);
        let sn = self.safety_for(&row);
        row.into_contact(sn, online)
    }

    // ---------------------------------------------------------------- contacts

    pub async fn add_contact(
        self: &Arc<Self>,
        card: ContactCard,
        verified: bool,
    ) -> Result<Contact> {
        let peer = qr::validate_card(&card)?;
        if peer == self.id.peer_id {
            bail!("cannot add yourself as a contact");
        }
        if let Some(existing) = self.store.get_contact(&card.peer_id)? {
            if existing.identity_key != card.identity_key
                || existing.ed25519_key != card.ed25519_key
            {
                bail!("this contact's keys changed; remove and re-add them only after verifying in person");
            }
        }
        let prev = self.store.get_contact(&card.peer_id)?;
        let row = ContactRow {
            peer_id: card.peer_id.clone(),
            display_name: card
                .display_name
                .clone()
                .filter(|n| !n.is_empty())
                .or(prev.as_ref().map(|p| p.display_name.clone()))
                .unwrap_or_default(),
            identity_key: card.identity_key.clone(),
            ed25519_key: card.ed25519_key.clone(),
            addrs: if card.addrs.is_empty() {
                prev.as_ref().map(|p| p.addrs.clone()).unwrap_or_default()
            } else {
                card.addrs.clone()
            },
            verified: verified || prev.as_ref().is_some_and(|p| p.verified),
            added_at: prev.as_ref().map(|p| p.added_at).unwrap_or_else(now_ms),
        };
        self.store.upsert_contact(&row)?;
        let handle = self.ns().handle.clone();
        if let Some(h) = handle {
            h.add_addrs(peer, parse_addrs(&row.addrs));
            // Introduce ourselves (and deliver anything queued).
            self.ns().greeted.remove(&peer);
            self.spawn_flush(peer);
        }
        Ok(self.to_contact(row))
    }

    pub fn list_contacts(&self) -> Result<Vec<Contact>> {
        Ok(self
            .store
            .list_contacts()?
            .into_iter()
            .map(|r| self.to_contact(r))
            .collect())
    }

    pub fn set_verified(&self, peer_id: &str, v: bool) -> Result<()> {
        self.store.set_verified(peer_id, v)
    }

    pub fn remove_contact(&self, peer_id: &str) -> Result<()> {
        self.crypto.forget_sessions(peer_id)?;
        self.store.remove_contact(peer_id)
    }

    // ---------------------------------------------------------------- messages

    pub async fn send_text(self: &Arc<Self>, peer_id: &str, body: String) -> Result<MessageId> {
        let peer = parse_peer(peer_id)?;
        if self.store.get_contact(peer_id)?.is_none() {
            bail!("unknown contact {peer_id}");
        }
        if body.is_empty() {
            bail!("empty message");
        }
        if body.len() > MAX_TEXT_BYTES {
            bail!("message too long (max {MAX_TEXT_BYTES} bytes)");
        }
        let now = now_ms();
        let msg = Message {
            id: uuid::Uuid::new_v4().to_string(),
            peer_id: peer_id.to_string(),
            outgoing: true,
            body,
            timestamp_ms: now,
            status: DeliveryStatus::Sent,
            read: true,
        };
        self.store.insert_message(&msg)?;
        self.store.outbox_add(&msg.id, peer_id, now)?;
        self.events.emit(CoreEvent::MessageStatus {
            id: msg.id.clone(),
            peer_id: msg.peer_id.clone(),
            status: DeliveryStatus::Sent,
        });
        self.spawn_flush(peer);
        Ok(msg.id)
    }

    pub fn list_conversations(&self) -> Result<Vec<ConversationSummary>> {
        self.store.list_conversations()
    }

    pub fn list_messages(
        &self,
        peer_id: &str,
        before: Option<i64>,
        limit: u32,
    ) -> Result<Vec<Message>> {
        self.store.list_messages(peer_id, before, limit.min(500))
    }

    pub fn mark_read(&self, peer_id: &str, up_to: &str) -> Result<()> {
        self.store.mark_read(peer_id, up_to)
    }

    pub async fn send_signal(
        self: &Arc<Self>,
        peer_id: &str,
        call_id: String,
        kind: SignalKind,
        payload: String,
    ) -> Result<()> {
        let peer = parse_peer(peer_id)?;
        let contact = self
            .store
            .get_contact(peer_id)?
            .ok_or_else(|| anyhow!("unknown contact {peer_id}"))?;
        if payload.len() > MAX_SIGNAL_BYTES || call_id.len() > 128 {
            bail!("signal too large");
        }
        let net = self.net_handle()?;
        let lock = self.peer_lock(peer);
        let fut = async {
            let _g = lock.lock().await;
            let p = Payload {
                v: 1,
                to: peer_id.to_string(),
                id: uuid::Uuid::new_v4().to_string(),
                ts: now_ms(),
                body: PayloadBody::Signal {
                    call_id,
                    kind,
                    payload,
                },
            };
            match self.deliver(&net, &contact, &p).await {
                Ok(()) => Ok(()),
                Err(DeliverError::NotAContact) => {
                    Err(anyhow!("peer has not accepted you as a contact yet"))
                }
                Err(DeliverError::Retry(e)) => Err(e.context("peer unreachable")),
            }
        };
        tokio::time::timeout(SIGNAL_TIMEOUT, fut)
            .await
            .map_err(|_| anyhow!("signal timed out"))?
    }

    // ---------------------------------------------------------------- network

    fn net_handle(&self) -> Result<NetHandle> {
        self.ns()
            .handle
            .clone()
            .ok_or_else(|| anyhow!("network not running; call start_network()"))
    }

    pub fn network_info(&self) -> NetworkInfo {
        let ns = self.ns();
        NetworkInfo {
            running: ns.handle.is_some(),
            listen_addrs: ns.listen_addrs.iter().map(|a| a.to_string()).collect(),
            connected_peers: ns.connected_count as u32,
            relay_reservations: ns.listen_addrs.iter().filter(|a| is_circuit(a)).count() as u32,
        }
    }

    fn emit_status(&self) {
        self.events.emit(CoreEvent::NetworkStatus {
            info: self.network_info(),
        });
    }

    pub async fn start_network(self: &Arc<Self>) -> Result<()> {
        if self.ns().handle.is_some() {
            return Ok(());
        }
        let mode = match &self.cfg.tor_socks {
            Some(s) => TransportMode::TorSocks(
                s.parse()
                    .with_context(|| format!("tor_socks must be ip:port, got {s}"))?,
            ),
            #[cfg(feature = "tor")]
            None => TransportMode::TorEmbedded,
            #[cfg(not(feature = "tor"))]
            None => TransportMode::Clearnet {
                mdns: self.cfg.enable_mdns,
            },
        };
        let parse = |v: &[String], what: &str| -> Result<Vec<Multiaddr>> {
            v.iter()
                .map(|a| {
                    a.parse::<Multiaddr>()
                        .with_context(|| format!("bad {what} addr {a}"))
                })
                .collect()
        };
        let listen = if self.cfg.listen_addrs.is_empty() {
            [
                "/ip4/0.0.0.0/tcp/0",
                "/ip4/0.0.0.0/udp/0/quic-v1",
                "/ip6/::/tcp/0",
                "/ip6/::/udp/0/quic-v1",
            ]
            .iter()
            .map(|s| s.parse().expect("static addr"))
            .collect()
        } else {
            parse(&self.cfg.listen_addrs, "listen")?
        };
        let known = self
            .store
            .list_contacts()?
            .into_iter()
            .filter_map(|c| Some((parse_peer(&c.peer_id).ok()?, parse_addrs(&c.addrs))))
            .collect();
        let cfg = NetConfig {
            mode,
            listen,
            bootstrap: parse(&self.cfg.bootstrap_addrs, "bootstrap")?,
            relays: parse(&self.cfg.relay_addrs, "relay")?,
            known,
        };
        let (handle, task) = net::spawn(self.clone(), self.id.keypair.clone(), cfg).await?;
        {
            let mut ns = self.ns();
            ns.handle = Some(handle);
            ns.task = Some(task);
        }
        self.emit_status();
        Ok(())
    }

    pub async fn stop_network(&self) -> Result<()> {
        let (handle, task) = {
            let mut ns = self.ns();
            (ns.handle.take(), ns.task.take())
        };
        if let Some(h) = handle {
            h.stop();
        }
        if let Some(t) = task {
            let _ = t.await;
        }
        Ok(())
    }

    // ---- callbacks from the swarm task (must not block) ----

    pub(crate) fn on_network_stopped(&self) {
        let peers: Vec<PeerId> = {
            let mut ns = self.ns();
            ns.handle = None;
            ns.listen_addrs.clear();
            ns.greeted.clear();
            ns.connected_count = 0;
            ns.connected.drain().collect()
        };
        for p in peers {
            if self
                .store
                .get_contact(&p.to_base58())
                .ok()
                .flatten()
                .is_some()
            {
                self.events.emit(CoreEvent::ContactOffline {
                    peer_id: p.to_base58(),
                });
            }
        }
        self.emit_status();
    }

    pub(crate) fn on_listen_addr(&self, addr: Multiaddr, added: bool, connected: usize) {
        {
            let mut ns = self.ns();
            ns.listen_addrs.retain(|a| a != &addr);
            if added {
                ns.listen_addrs.push(addr);
            }
            ns.connected_count = connected;
        }
        self.emit_status();
    }

    fn is_contact(&self, peer: &PeerId) -> bool {
        self.store
            .get_contact(&peer.to_base58())
            .ok()
            .flatten()
            .is_some()
    }

    pub(crate) fn on_connected(self: &Arc<Self>, peer: PeerId, connected: usize) {
        {
            let mut ns = self.ns();
            ns.connected.insert(peer);
            ns.connected_count = connected;
        }
        if self.is_contact(&peer) {
            self.events.emit(CoreEvent::ContactOnline {
                peer_id: peer.to_base58(),
            });
            self.spawn_flush(peer);
        }
        self.emit_status();
    }

    pub(crate) fn on_disconnected(&self, peer: PeerId, connected: usize) {
        {
            let mut ns = self.ns();
            ns.connected.remove(&peer);
            ns.greeted.remove(&peer);
            ns.connected_count = connected;
        }
        if self.is_contact(&peer) {
            self.events.emit(CoreEvent::ContactOffline {
                peer_id: peer.to_base58(),
            });
        }
        self.emit_status();
    }

    pub(crate) fn on_discovered(self: &Arc<Self>, peer: PeerId) {
        if self.has_outbox(&peer) {
            self.spawn_flush(peer);
        }
    }

    pub(crate) fn on_tick(self: &Arc<Self>) {
        if let Ok(expired) = self.store.outbox_expire(now_ms() - OUTBOX_TTL_MS) {
            for (id, peer_id) in expired {
                let _ = self.store.set_message_status(&id, DeliveryStatus::Failed);
                self.events.emit(CoreEvent::MessageStatus {
                    id,
                    peer_id,
                    status: DeliveryStatus::Failed,
                });
            }
        }
        for p in self.store.outbox_peers().unwrap_or_default() {
            if let Ok(peer) = parse_peer(&p) {
                self.spawn_flush(peer);
            }
        }
    }

    fn has_outbox(&self, peer: &PeerId) -> bool {
        self.store
            .outbox_for(&peer.to_base58())
            .map(|v| !v.is_empty())
            .unwrap_or(false)
    }

    // ---- inbound ----

    pub(crate) fn handle_msg(self: &Arc<Self>, peer: PeerId, req: MsgRequest) -> MsgResponse {
        match req {
            MsgRequest::Hello { card } => self.on_hello(peer, card),
            MsgRequest::Olm { t, c } => match self.on_olm(peer, t, &c) {
                Ok(r) => r,
                Err(e) => {
                    tracing::warn!(%peer, "inbound message rejected: {e:#}");
                    MsgResponse::Rejected(RejectReason::DecryptFailed)
                }
            },
        }
    }

    fn on_hello(self: &Arc<Self>, peer: PeerId, card: ContactCard) -> MsgResponse {
        match qr::validate_card(&card) {
            Ok(p) if p == peer => {}
            _ => return MsgResponse::Rejected(RejectReason::Malformed),
        }
        match self.store.get_contact(&card.peer_id) {
            Ok(Some(c)) => {
                if c.identity_key != card.identity_key || c.ed25519_key != card.ed25519_key {
                    tracing::warn!(%peer, "hello with changed keys ignored");
                    return MsgResponse::Rejected(RejectReason::Malformed);
                }
                let addrs = (!card.addrs.is_empty()).then_some(card.addrs.as_slice());
                let _ = self.store.update_contact_seen(
                    &card.peer_id,
                    card.display_name.as_deref(),
                    addrs,
                );
                if let Some(h) = self.ns().handle.clone() {
                    h.add_addrs(peer, parse_addrs(&card.addrs));
                }
                if self.has_outbox(&peer) {
                    self.spawn_flush(peer);
                }
                MsgResponse::Ok
            }
            Ok(None) => {
                self.events.emit(CoreEvent::ContactRequest { card });
                MsgResponse::Rejected(RejectReason::NotAContact)
            }
            Err(_) => MsgResponse::Rejected(RejectReason::Internal),
        }
    }

    fn on_olm(self: &Arc<Self>, peer: PeerId, t: u8, c: &[u8]) -> Result<MsgResponse> {
        let pid = peer.to_base58();
        let Some(contact) = self.store.get_contact(&pid)? else {
            return Ok(MsgResponse::Rejected(RejectReason::NotAContact));
        };
        let ik = Curve25519PublicKey::from_base64(&contact.identity_key)?;
        let pt = self.crypto.decrypt(&pid, ik, t, c)?;
        let p: Payload = match serde_json::from_slice(&pt) {
            Ok(p) => p,
            Err(_) => return Ok(MsgResponse::Rejected(RejectReason::Malformed)),
        };
        if p.to != self.id.peer_id.to_base58() {
            return Ok(MsgResponse::Rejected(RejectReason::Malformed));
        }
        match p.body {
            PayloadBody::Text { text } => {
                if text.len() > MAX_TEXT_BYTES {
                    return Ok(MsgResponse::Rejected(RejectReason::Malformed));
                }
                let m = Message {
                    id: p.id,
                    peer_id: pid,
                    outgoing: false,
                    body: text,
                    timestamp_ms: p.ts,
                    status: DeliveryStatus::Received,
                    read: false,
                };
                if self.store.insert_message(&m)? {
                    self.events.emit(CoreEvent::MessageReceived { message: m });
                }
            }
            PayloadBody::Signal {
                call_id,
                kind,
                payload,
            } => {
                self.events.emit(CoreEvent::Signal {
                    peer_id: pid,
                    call_id,
                    kind,
                    payload,
                });
            }
        }
        if self.has_outbox(&peer) {
            self.spawn_flush(peer);
        }
        Ok(MsgResponse::Delivered)
    }

    pub(crate) fn handle_keys(&self, peer: PeerId, req: KeysRequest) -> KeysResponse {
        match req {
            KeysRequest::ClaimOneTimeKey => {
                if !self.is_contact(&peer) {
                    return KeysResponse::Refused(RejectReason::NotAContact);
                }
                match self.crypto.claim_one_time_key() {
                    Ok(k) => KeysResponse::Key {
                        identity_key: self.id.curve25519_b64(),
                        one_time_key: k.to_base64(),
                    },
                    Err(e) => {
                        tracing::error!("one-time key: {e:#}");
                        KeysResponse::Refused(RejectReason::Internal)
                    }
                }
            }
        }
    }

    // ---- outbound ----

    fn peer_lock(&self, peer: PeerId) -> Arc<tokio::sync::Mutex<()>> {
        self.peer_locks
            .lock()
            .expect("locks poisoned")
            .entry(peer)
            .or_default()
            .clone()
    }

    fn spawn_flush(self: &Arc<Self>, peer: PeerId) {
        if self.ns().handle.is_none() {
            return;
        }
        let me = self.clone();
        tokio::spawn(async move {
            if let Err(e) = me.flush(peer).await {
                tracing::debug!(%peer, "flush: {e:#}");
            }
        });
    }

    async fn greet(&self, net: &NetHandle, peer: PeerId) -> Result<()> {
        if self.ns().greeted.contains(&peer) {
            return Ok(());
        }
        let resp = net
            .msg(
                peer,
                MsgRequest::Hello {
                    card: self.self_card(),
                },
            )
            .await?;
        tracing::debug!(%peer, ?resp, "hello");
        self.ns().greeted.insert(peer);
        Ok(())
    }

    async fn ensure_session(
        &self,
        net: &NetHandle,
        contact: &ContactRow,
        peer: PeerId,
    ) -> Result<(), DeliverError> {
        if self.crypto.has_session(&contact.peer_id)? {
            return Ok(());
        }
        match net.keys(peer, KeysRequest::ClaimOneTimeKey).await? {
            KeysResponse::Key {
                identity_key,
                one_time_key,
            } => {
                if identity_key != contact.identity_key {
                    return Err(DeliverError::Retry(anyhow!(
                        "peer presented a different identity key"
                    )));
                }
                let ik =
                    Curve25519PublicKey::from_base64(&identity_key).map_err(anyhow::Error::from)?;
                let otk =
                    Curve25519PublicKey::from_base64(&one_time_key).map_err(anyhow::Error::from)?;
                self.crypto.create_outbound(&contact.peer_id, ik, otk)?;
                Ok(())
            }
            KeysResponse::Refused(RejectReason::NotAContact) => Err(DeliverError::NotAContact),
            KeysResponse::Refused(r) => {
                Err(DeliverError::Retry(anyhow!("key claim refused: {r:?}")))
            }
        }
    }

    /// Encrypt and deliver one payload; resets the session once if the peer
    /// can't decrypt (e.g. it reinstalled).
    async fn deliver(
        &self,
        net: &NetHandle,
        contact: &ContactRow,
        p: &Payload,
    ) -> Result<(), DeliverError> {
        let peer = parse_peer(&contact.peer_id)?;
        self.greet(net, peer).await?;
        let pt = serde_json::to_vec(p).map_err(anyhow::Error::from)?;
        for _ in 0..2 {
            self.ensure_session(net, contact, peer).await?;
            let (t, c) = self.crypto.encrypt(&contact.peer_id, &pt)?;
            match net.msg(peer, MsgRequest::Olm { t, c }).await? {
                MsgResponse::Delivered => return Ok(()),
                MsgResponse::Rejected(RejectReason::NotAContact) => {
                    return Err(DeliverError::NotAContact)
                }
                MsgResponse::Rejected(RejectReason::DecryptFailed) => {
                    tracing::info!(%peer, "peer could not decrypt; starting a new Olm session");
                    self.crypto.forget_sessions(&contact.peer_id)?;
                }
                other => {
                    return Err(DeliverError::Retry(anyhow!(
                        "unexpected response {other:?}"
                    )))
                }
            }
        }
        Err(DeliverError::Retry(anyhow!(
            "could not establish a working session"
        )))
    }

    /// Greet the peer and deliver its outbox in order.
    async fn flush(self: &Arc<Self>, peer: PeerId) -> Result<()> {
        let lock = self.peer_lock(peer);
        let Ok(_g) = lock.try_lock() else {
            return Ok(()); // another flush is running; it will pick up new entries
        };
        let pid = peer.to_base58();
        let Some(contact) = self.store.get_contact(&pid)? else {
            return Ok(());
        };
        let net = self.net_handle()?;
        if let Err(e) = self.greet(&net, peer).await {
            return Err(e.context("hello failed"));
        }
        loop {
            let rows = self.store.outbox_for(&pid)?;
            if rows.is_empty() {
                return Ok(());
            }
            for row in rows {
                let Some(m) = self.store.get_message(&row.msg_id)? else {
                    self.store.outbox_remove(&row.msg_id)?;
                    continue;
                };
                self.store.outbox_attempt(&row.msg_id, now_ms())?;
                let p = Payload {
                    v: 1,
                    to: pid.clone(),
                    id: m.id.clone(),
                    ts: m.timestamp_ms,
                    body: PayloadBody::Text { text: m.body },
                };
                match self.deliver(&net, &contact, &p).await {
                    Ok(()) => {
                        self.store.outbox_remove(&m.id)?;
                        self.store
                            .set_message_status(&m.id, DeliveryStatus::Delivered)?;
                        self.events.emit(CoreEvent::MessageStatus {
                            id: m.id,
                            peer_id: pid.clone(),
                            status: DeliveryStatus::Delivered,
                        });
                    }
                    Err(DeliverError::NotAContact) => return Ok(()),
                    Err(DeliverError::Retry(e)) => return Err(e),
                }
            }
        }
    }
}
