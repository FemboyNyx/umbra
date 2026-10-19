//! The core engine: ties identity, store, Olm and the network together.
//!
//! Usable directly from Rust (tests run several instances in one process);
//! the FFI layer in `crate::api` wraps a single global instance.

use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use anyhow::{anyhow, bail, Context, Result};
use libp2p::multiaddr::Protocol;
use libp2p::{Multiaddr, PeerId};
use vodozemac::Curve25519PublicKey;

use crate::api::types::*;
use crate::engine::crypto::{fingerprint_of, Crypto, Identity};
use crate::engine::events::EventHub;
use crate::engine::limits::{KeyedBuckets, Limits, TokenBucket};
use crate::engine::net::{self, NetConfig, NetHandle, TransportMode};
use crate::engine::proto::*;
use crate::engine::servers::*;
use crate::engine::store::{CoMemberRow, ContactRow, MembershipRow, OutboxItem, Store};
use crate::engine::util::{b64, now_ms, unb64};
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
    limits: Limits,
    limiter: Mutex<Limiter>,
    #[cfg_attr(not(feature = "tor"), allow(dead_code))]
    data_dir: std::path::PathBuf,
    /// Engine-internal consumers of server payloads (Phase 1 server logic,
    /// tests). Never crosses the FFI.
    server_subs: Mutex<Vec<tokio::sync::mpsc::UnboundedSender<ServerInbound>>>,
}

/// Everything we trust about one peer's keys, across its contact record and
/// all its co-member records. The contact pin is authoritative; co-member
/// records are kept equal to it (mismatches raise `KeyChanged`).
struct Pin {
    peer_id: String,
    ed25519_key: String,
    identity_key: String,
    contact: Option<ContactRow>,
    rows: Vec<CoMemberRow>,
}

impl Pin {
    fn knows(&self, key: &str) -> bool {
        let c = self.contact.iter().any(|c| {
            c.pending_identity_key.as_deref() == Some(key)
                || c.rejected_identity_key.as_deref() == Some(key)
        });
        c || self.rows.iter().any(|r| {
            r.pending_identity_key.as_deref() == Some(key)
                || r.rejected_identity_key.as_deref() == Some(key)
        })
    }

    fn key_change_pending(&self) -> bool {
        self.contact
            .iter()
            .any(|c| c.pending_identity_key.is_some())
            || self.rows.iter().any(|r| r.pending_identity_key.is_some())
    }

    /// Server traffic for `server_id` is allowed: a co-member record exists,
    /// is signed, no key change is pending anywhere, and all copies agree.
    fn server_ok(&self, server_id: &str) -> bool {
        !self.key_change_pending()
            && self
                .rows
                .iter()
                .any(|r| r.server_id == server_id && r.card_sig.is_some())
            && self
                .rows
                .iter()
                .all(|r| r.identity_key == self.identity_key)
    }
}

/// Mutable rate-limiter state (see `engine::limits`).
struct Limiter {
    hello: KeyedBuckets<PeerId>,
    otk_claims: KeyedBuckets<PeerId>,
    contact_requests: TokenBucket,
    /// Unknown identities with an emitted `ContactRequest` -> last emission.
    pending_requests: HashMap<PeerId, Instant>,
    /// Fresh OTKs issued per requester and not yet consumed.
    outstanding_otks: HashMap<PeerId, Vec<(Curve25519PublicKey, Instant)>>,
}

impl Limiter {
    fn new(l: &Limits) -> Self {
        Self {
            hello: KeyedBuckets::new(l.hello_burst, l.hello_refill, l.max_tracked_peers),
            otk_claims: KeyedBuckets::new(
                l.otk_claim_burst,
                l.otk_claim_refill,
                l.max_tracked_peers,
            ),
            contact_requests: TokenBucket::new(l.contact_request_burst, l.contact_request_refill),
            pending_requests: HashMap::new(),
            outstanding_otks: HashMap::new(),
        }
    }
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
        Self::open_with_limits(data_dir, db_key, cfg, events, Limits::default())
    }

    /// [`Self::open`] with non-default abuse limits.
    pub fn open_with_limits(
        data_dir: &Path,
        db_key: &[u8],
        cfg: CoreConfig,
        events: EventHub,
        limits: Limits,
    ) -> Result<Arc<Self>> {
        std::fs::create_dir_all(data_dir)
            .with_context(|| format!("create {}", data_dir.display()))?;
        let store = Arc::new(Store::open(&data_dir.join(DB_FILE), db_key)?);
        let (id, crypto) = Crypto::load_or_create(store.clone(), db_key)?;
        Ok(Arc::new(Self {
            data_dir: data_dir.to_path_buf(),
            id,
            store,
            crypto,
            events,
            cfg,
            net: Mutex::new(NetState::default()),
            peer_locks: Mutex::new(HashMap::new()),
            limiter: Mutex::new(Limiter::new(&limits)),
            limits,
            server_subs: Mutex::new(Vec::new()),
        }))
    }

    fn lim(&self) -> MutexGuard<'_, Limiter> {
        self.limiter.lock().expect("limiter poisoned")
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
        let mut card = ContactCard {
            peer_id: self.id.peer_id.to_base58(),
            identity_key: self.id.curve25519_b64(),
            ed25519_key: self.id.ed25519_b64(),
            addrs: self.advertised_addrs(),
            display_name: (!name.is_empty()).then_some(name),
            sig: None,
        };
        // Card v2: bind our Olm identity key to the device key.
        if let Err(e) = qr::sign_card(&self.id.keypair, &mut card) {
            tracing::error!("sign own card: {e:#}");
        }
        card
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
        self.safety_with_key(c, &c.identity_key)
    }

    /// Safety number this contact would have with `identity_key`.
    fn safety_with_key(&self, c: &ContactRow, identity_key: &str) -> String {
        self.safety_for_keys(&c.peer_id, &c.ed25519_key, identity_key)
    }

    fn safety_for_keys(&self, peer_id: &str, ed25519_key: &str, identity_key: &str) -> String {
        let theirs = (|| -> Result<String> {
            let peer = parse_peer(peer_id)?;
            let ed = unb64(ed25519_key)?;
            let ik = Curve25519PublicKey::from_base64(identity_key)?;
            Ok(fingerprint_of(&peer, &ed, &ik))
        })()
        .unwrap_or_default();
        safety::safety_number(&self.id.fp_digits, &theirs)
    }

    /// Current pin for `peer_id`, if it is a contact and/or a co-member.
    fn pin(&self, peer_id: &str) -> Result<Option<Pin>> {
        let contact = self.store.get_contact(peer_id)?;
        let rows = self.store.co_member_rows(peer_id)?;
        let (ed, ik) = match (&contact, rows.first()) {
            (Some(c), _) => (c.ed25519_key.clone(), c.identity_key.clone()),
            (None, Some(r)) => (r.ed25519_key.clone(), r.identity_key.clone()),
            (None, None) => return Ok(None),
        };
        Ok(Some(Pin {
            peer_id: peer_id.to_string(),
            ed25519_key: ed,
            identity_key: ik,
            contact,
            rows,
        }))
    }

    /// A known peer (contact and/or co-member) presented `presented_ik`
    /// instead of its trusted key. Never trusts it: flags every record of the
    /// peer and emits `KeyChanged` once per distinct key. Keys the user
    /// rejected are ignored silently.
    ///
    /// `presented_sig` is the card v2 signature that came with the new key
    /// (already verified by `validate_card`), if any. Co-member records only
    /// take signed keys; an unsigned change of a co-member-only peer is
    /// refused without an alert (no evidence it came from the device key).
    fn on_key_mismatch(
        &self,
        pin: &Pin,
        presented_ik: &str,
        presented_sig: Option<&str>,
        source: KeyChangeSource,
    ) {
        if Curve25519PublicKey::from_base64(presented_ik).is_err() {
            return;
        }
        tracing::warn!(peer = %pin.peer_id, ?source, "peer presented a different identity key");
        if pin.knows(presented_ik) {
            return;
        }
        let mut flagged = false;
        if let Some(c) = &pin.contact {
            match self
                .store
                .set_pending_key(&c.peer_id, presented_ik, presented_sig, now_ms())
            {
                Ok(()) => flagged = true,
                Err(e) => tracing::error!("store pending key: {e:#}"),
            }
        }
        if let (false, Some(sig)) = (pin.rows.is_empty(), presented_sig) {
            match self
                .store
                .co_member_set_pending(&pin.peer_id, presented_ik, sig)
            {
                Ok(()) => flagged = true,
                Err(e) => tracing::error!("store pending co-member key: {e:#}"),
            }
        }
        if !flagged {
            return;
        }
        self.events.emit(CoreEvent::KeyChanged {
            peer_id: pin.peer_id.clone(),
            old_identity_key: pin.identity_key.clone(),
            new_identity_key: presented_ik.to_string(),
            old_safety_number: self.safety_for_keys(
                &pin.peer_id,
                &pin.ed25519_key,
                &pin.identity_key,
            ),
            new_safety_number: self.safety_for_keys(&pin.peer_id, &pin.ed25519_key, presented_ik),
            source,
        });
    }

    /// Trust the peer's pending new identity key (contact and/or co-member
    /// records together). Old Olm sessions are dropped, a contact becomes
    /// unverified, and queued messages are re-sent under the new key.
    /// Contacts only (errors without side effects otherwise); co-member-only
    /// peers use [`Self::accept_key_change_any`].
    pub fn accept_key_change(self: &Arc<Self>, peer_id: &str) -> Result<Contact> {
        if self.store.get_contact(peer_id)?.is_none() {
            bail!("{peer_id} is not a contact");
        }
        self.accept_key_change_any(peer_id)?;
        let row = self
            .store
            .get_contact(peer_id)?
            .ok_or_else(|| anyhow!("unknown contact {peer_id}"))?;
        Ok(self.to_contact(row))
    }

    /// [`Self::accept_key_change`] for any pinned peer.
    pub fn accept_key_change_any(self: &Arc<Self>, peer_id: &str) -> Result<()> {
        let peer = parse_peer(peer_id)?;
        let pin = self
            .pin(peer_id)?
            .ok_or_else(|| anyhow!("unknown peer {peer_id}"))?;
        let (key, sig) = if let Some(c) = pin
            .contact
            .as_ref()
            .filter(|c| c.pending_identity_key.is_some())
        {
            let key = c.pending_identity_key.clone().expect("checked");
            let sig = c.pending_card_sig.clone();
            self.store.accept_pending_key(peer_id)?;
            (key, sig)
        } else if let Some(r) = pin.rows.iter().find(|r| r.pending_identity_key.is_some()) {
            (
                r.pending_identity_key.clone().expect("checked"),
                r.pending_card_sig.clone(),
            )
        } else {
            bail!("no pending key change for {peer_id}");
        };
        if !pin.rows.is_empty() {
            self.store
                .co_member_set_key(peer_id, &key, sig.as_deref())?;
        }
        self.crypto.forget_sessions(peer_id)?;
        self.ns().greeted.remove(&peer);
        self.spawn_flush(peer);
        Ok(())
    }

    /// Keep the old key. The refused key is remembered and won't alert again;
    /// a different new key will.
    pub fn reject_key_change(&self, peer_id: &str) -> Result<()> {
        let pin = self
            .pin(peer_id)?
            .ok_or_else(|| anyhow!("unknown peer {peer_id}"))?;
        if !pin.key_change_pending() {
            bail!("no pending key change for {peer_id}");
        }
        if pin
            .contact
            .as_ref()
            .is_some_and(|c| c.pending_identity_key.is_some())
        {
            self.store.reject_pending_key(peer_id)?;
        }
        self.store.co_member_reject(peer_id)
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
        if let Some(pin) = self.pin(&card.peer_id)? {
            // Same PeerId implies same ed25519 key (validated), so only the
            // Olm identity key can differ. Pins carry over from co-member
            // records too (a co-member being added as a contact).
            let signed = pin
                .contact
                .as_ref()
                .map_or(!pin.rows.is_empty(), |c| c.card_sig.is_some());
            if signed && card.sig.is_none() {
                bail!(
                    "unsigned (v1) card for a peer we hold a signed card for; refusing downgrade"
                );
            }
            if pin.identity_key != card.identity_key {
                self.on_key_mismatch(
                    &pin,
                    &card.identity_key,
                    card.sig.as_deref(),
                    KeyChangeSource::ContactCard,
                );
                bail!(
                    "this contact's identity key changed; compare safety numbers, then call \
                     accept_key_change or reject_key_change"
                );
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
            pending_identity_key: prev.as_ref().and_then(|p| p.pending_identity_key.clone()),
            rejected_identity_key: prev.as_ref().and_then(|p| p.rejected_identity_key.clone()),
            card_sig: card
                .sig
                .clone()
                .or(prev.as_ref().and_then(|p| p.card_sig.clone())),
            pending_card_sig: prev.as_ref().and_then(|p| p.pending_card_sig.clone()),
        };
        self.store.upsert_contact(&row)?;
        self.lim().pending_requests.remove(&peer);
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
        // The Olm channel stays if we still share a server with the peer.
        let co_member = !self.store.co_member_rows(peer_id)?.is_empty();
        if !co_member {
            self.crypto.forget_sessions(peer_id)?;
        }
        self.store.remove_contact(peer_id, co_member)
    }

    // ---------------------------------------------------------------- servers

    /// Record that we are in `server_id`, using `identity` (explicit; see
    /// [`ServerIdentity`]). Idempotent for the same identity; the identity of
    /// a membership can't change. `history` is stored only.
    pub fn add_server_membership(
        &self,
        server_id: &str,
        identity: ServerIdentity,
        history: Option<HistoryVisibility>,
    ) -> Result<ServerMembership> {
        validate_server_id(server_id)?;
        if identity == ServerIdentity::Dedicated {
            bail!("per-server (dedicated) identities are not supported yet");
        }
        if let Some(m) = self.store.get_membership(server_id)? {
            if m.identity != identity.as_str() {
                bail!(
                    "server {server_id} already uses the {} identity",
                    m.identity
                );
            }
        }
        self.store.upsert_membership(&MembershipRow {
            server_id: server_id.to_string(),
            identity: identity.as_str().to_string(),
            history_visibility: history.map(|h| h.as_str().to_string()),
            joined_at: now_ms(),
        })?;
        self.server_membership(server_id)?
            .ok_or_else(|| anyhow!("membership vanished"))
    }

    pub fn server_membership(&self, server_id: &str) -> Result<Option<ServerMembership>> {
        Ok(self
            .store
            .get_membership(server_id)?
            .map(|m| ServerMembership {
                identity: ServerIdentity::parse(&m.identity).unwrap_or(ServerIdentity::Default),
                history_visibility: m
                    .history_visibility
                    .as_deref()
                    .and_then(HistoryVisibility::parse),
                server_id: m.server_id,
                joined_at_ms: m.joined_at,
            }))
    }

    /// Per-server history visibility for new members (stored only).
    pub fn set_server_history(
        &self,
        server_id: &str,
        history: Option<HistoryVisibility>,
    ) -> Result<()> {
        let mut m = self
            .store
            .get_membership(server_id)?
            .ok_or_else(|| anyhow!("not a member of {server_id}"))?;
        m.history_visibility = history.map(|h| h.as_str().to_string());
        self.store.upsert_membership(&m)
    }

    /// Add a co-member of `server_id` from its roster card. The card must be
    /// card v2 (signed). Never creates a contact. If we already pin a
    /// different key for this peer (as a contact or in another server) the
    /// card is refused and `KeyChanged { source: Roster }` is raised.
    pub fn add_co_member(self: &Arc<Self>, server_id: &str, card: ContactCard) -> Result<CoMember> {
        if self.store.get_membership(server_id)?.is_none() {
            bail!("not a member of {server_id}");
        }
        let peer = qr::validate_card(&card)?;
        let Some(sig) = card.sig.clone() else {
            bail!("co-members need a signed (v2) card");
        };
        if peer == self.id.peer_id {
            bail!("cannot add yourself as a co-member");
        }
        if let Some(pin) = self.pin(&card.peer_id)? {
            if pin.identity_key != card.identity_key {
                self.on_key_mismatch(
                    &pin,
                    &card.identity_key,
                    Some(&sig),
                    KeyChangeSource::Roster,
                );
                bail!(
                    "this member's identity key differs from the one we trust; compare safety \
                     numbers, then call accept_key_change or reject_key_change"
                );
            }
        }
        let row = CoMemberRow {
            server_id: server_id.to_string(),
            peer_id: card.peer_id.clone(),
            display_name: card.display_name.clone().unwrap_or_default(),
            identity_key: card.identity_key.clone(),
            ed25519_key: card.ed25519_key.clone(),
            card_sig: Some(sig),
            addrs: card.addrs.clone(),
            added_at: now_ms(),
            pending_identity_key: None,
            pending_card_sig: None,
            rejected_identity_key: None,
        };
        self.store.upsert_co_member(&row)?;
        // Records of the same peer share key-change state.
        if let Some(p) = self
            .store
            .co_member_rows(&card.peer_id)?
            .into_iter()
            .find(|r| r.pending_identity_key.is_some())
        {
            if let (Some(k), Some(s)) = (&p.pending_identity_key, &p.pending_card_sig) {
                self.store.co_member_set_pending(&card.peer_id, k, s)?;
            }
        }
        if let Some(h) = self.ns().handle.clone() {
            h.add_addrs(peer, parse_addrs(&card.addrs));
        }
        self.co_member(server_id, &card.peer_id)?
            .ok_or_else(|| anyhow!("co-member vanished"))
    }

    /// Remove one co-member record. When the peer is in no shared server and
    /// is not a contact, its Olm sessions are dropped too.
    pub fn remove_co_member(&self, server_id: &str, peer_id: &str) -> Result<()> {
        if !self.store.remove_co_member(server_id, peer_id)? {
            bail!("{peer_id} is not a co-member of {server_id}");
        }
        if self.store.co_member_rows(peer_id)?.is_empty()
            && self.store.get_contact(peer_id)?.is_none()
        {
            self.crypto.forget_sessions(peer_id)?;
        }
        Ok(())
    }

    pub fn co_member(&self, server_id: &str, peer_id: &str) -> Result<Option<CoMember>> {
        Ok(self
            .list_co_members(Some(server_id))?
            .into_iter()
            .find(|m| m.peer_id == peer_id))
    }

    /// Co-members of one server, or of all servers.
    pub fn list_co_members(&self, server_id: Option<&str>) -> Result<Vec<CoMember>> {
        Ok(self
            .store
            .list_co_members(server_id)?
            .into_iter()
            .map(|r| CoMember {
                is_contact: self.store.get_contact(&r.peer_id).ok().flatten().is_some(),
                signed_card: r.card_sig.is_some(),
                key_changed: r.pending_identity_key.is_some(),
                server_id: r.server_id,
                peer_id: r.peer_id,
                display_name: r.display_name,
                identity_key: r.identity_key,
                ed25519_key: r.ed25519_key,
                addrs: r.addrs,
                added_at_ms: r.added_at,
                pending_identity_key: r.pending_identity_key,
            })
            .collect())
    }

    /// Receive server payloads from co-members (engine-internal).
    pub fn subscribe_server(&self) -> tokio::sync::mpsc::UnboundedReceiver<ServerInbound> {
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        self.server_subs
            .lock()
            .expect("server subs poisoned")
            .push(tx);
        rx
    }

    fn emit_server(&self, msg: ServerInbound) {
        self.server_subs
            .lock()
            .expect("server subs poisoned")
            .retain(|tx| tx.send(msg.clone()).is_ok());
    }

    fn is_known_peer(&self, peer: &PeerId) -> bool {
        let pid = peer.to_base58();
        self.is_contact(peer)
            || self
                .store
                .co_member_rows(&pid)
                .map(|v| !v.is_empty())
                .unwrap_or(false)
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
            .pin(peer_id)?
            .filter(|p| p.contact.is_some())
            .ok_or_else(|| anyhow!("unknown contact {peer_id}"))?;
        if payload.len() > MAX_SIGNAL_BYTES || call_id.len() > 128 {
            bail!("signal too large");
        }
        let net = self.net_handle()?;
        let lock = self.peer_lock(peer);
        let fut = async {
            let _g = lock.lock().await;
            let p = Payload::new(
                peer_id.to_string(),
                uuid::Uuid::new_v4().to_string(),
                now_ms(),
                PayloadBody::Signal {
                    call_id,
                    kind,
                    payload,
                },
                EnvelopeMeta::default(),
            );
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
            None => {
                // `<data_dir>/tor/{state,cache}`. state holds arti guards and
                // (tor-listen) the onion identity keystore: wiping it changes
                // the .onion, so back it up with the Olm store.
                let root = self.data_dir.join("tor");
                let (state_dir, cache_dir) = (root.join("state"), root.join("cache"));
                for d in [&root, &state_dir, &cache_dir] {
                    crate::engine::net::private_dir(d)?;
                }
                TransportMode::TorEmbedded {
                    state_dir,
                    cache_dir,
                }
            }
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
        } else if self.has_outbox(&peer) {
            self.spawn_flush(peer); // co-member with queued server payloads
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
            for row in expired {
                match row.item {
                    OutboxItem::Dm => {
                        let _ = self
                            .store
                            .set_message_status(&row.msg_id, DeliveryStatus::Failed);
                        self.events.emit(CoreEvent::MessageStatus {
                            id: row.msg_id,
                            peer_id: row.peer_id,
                            status: DeliveryStatus::Failed,
                        });
                    }
                    // No user-visible message; Phase 1 decides what to surface.
                    OutboxItem::Server { .. } => {
                        tracing::debug!("expired an undelivered server payload");
                    }
                }
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
        if !self.lim().hello.try_take(&peer, Instant::now()) {
            return MsgResponse::Rejected(RejectReason::RateLimited);
        }
        match qr::validate_card(&card) {
            Ok(p) if p == peer => {}
            _ => return MsgResponse::Rejected(RejectReason::Malformed),
        }
        let pin = match self.pin(&card.peer_id) {
            Ok(p) => p,
            Err(_) => return MsgResponse::Rejected(RejectReason::Internal),
        };
        // Co-member that is not a contact: completes Hello, never becomes a
        // contact and never raises ContactRequest.
        if let Some(pin) = pin.as_ref().filter(|p| p.contact.is_none()) {
            if card.sig.is_none() {
                return MsgResponse::Rejected(RejectReason::Malformed);
            }
            if pin.identity_key != card.identity_key {
                self.on_key_mismatch(
                    pin,
                    &card.identity_key,
                    card.sig.as_deref(),
                    KeyChangeSource::Hello,
                );
                return MsgResponse::Rejected(RejectReason::KeyMismatch);
            }
            let _ = self
                .store
                .co_member_seen(&card.peer_id, &card.addrs, card.sig.as_deref());
            if let Some(h) = self.ns().handle.clone() {
                h.add_addrs(peer, parse_addrs(&card.addrs));
            }
            if self.has_outbox(&peer) {
                self.spawn_flush(peer);
            }
            return MsgResponse::Ok;
        }
        match pin.and_then(|p| p.contact.clone().map(|c| (p, c))) {
            Some((pin, c)) => {
                // No downgrade: once we hold a signed card, unsigned ones are refused.
                if c.card_sig.is_some() && card.sig.is_none() {
                    return MsgResponse::Rejected(RejectReason::Malformed);
                }
                if c.identity_key != card.identity_key {
                    self.on_key_mismatch(
                        &pin,
                        &card.identity_key,
                        card.sig.as_deref(),
                        KeyChangeSource::Hello,
                    );
                    return MsgResponse::Rejected(RejectReason::KeyMismatch);
                }
                if !pin.rows.is_empty() {
                    let _ =
                        self.store
                            .co_member_seen(&card.peer_id, &card.addrs, card.sig.as_deref());
                }
                // Upgrade a legacy contact to a signed card (already verified).
                if let Some(sig) = card.sig.as_deref() {
                    if c.card_sig.as_deref() != Some(sig) {
                        let _ = self.store.set_card_sig(&c.peer_id, sig);
                    }
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
            None => {
                if self.admit_contact_request(peer) {
                    self.events.emit(CoreEvent::ContactRequest { card });
                }
                MsgResponse::Rejected(RejectReason::NotAContact)
            }
        }
    }

    /// Dedupe + budget for `ContactRequest` events from unknown identities.
    fn admit_contact_request(&self, peer: PeerId) -> bool {
        let now = Instant::now();
        let l = &self.limits;
        let mut lim = self.lim();
        let ttl = l.pending_contact_request_ttl;
        lim.pending_requests
            .retain(|_, t| now.saturating_duration_since(*t) < ttl);
        match lim.pending_requests.get(&peer) {
            Some(t) if now.saturating_duration_since(*t) < l.contact_request_window => false,
            Some(_) => {
                if lim.contact_requests.try_take(now) {
                    lim.pending_requests.insert(peer, now);
                    true
                } else {
                    false
                }
            }
            None => {
                if lim.pending_requests.len() >= l.max_pending_contact_requests {
                    tracing::warn!("pending contact requests full; dropping request from {peer}");
                    return false;
                }
                if !lim.contact_requests.try_take(now) {
                    tracing::warn!(
                        "contact request budget exhausted; dropping request from {peer}"
                    );
                    return false;
                }
                lim.pending_requests.insert(peer, now);
                true
            }
        }
    }

    fn on_olm(self: &Arc<Self>, peer: PeerId, t: u8, c: &[u8]) -> Result<MsgResponse> {
        let pid = peer.to_base58();
        let Some(pin) = self.pin(&pid)? else {
            return Ok(MsgResponse::Rejected(RejectReason::NotAContact));
        };
        let ik = Curve25519PublicKey::from_base64(&pin.identity_key)?;
        let (pt, consumed) = self.crypto.decrypt_ex(&pid, ik, t, c)?;
        if let Some(k) = consumed {
            if let Some(v) = self.lim().outstanding_otks.get_mut(&peer) {
                v.retain(|(x, _)| *x != k);
            }
        }
        let p: Payload = match decode_payload(&pt) {
            Ok(p) => p,
            Err(_) => return Ok(MsgResponse::Rejected(RejectReason::Malformed)),
        };
        if p.to != self.id.peer_id.to_base58() {
            return Ok(MsgResponse::Rejected(RejectReason::Malformed));
        }
        // DMs and calls are for contacts only. A co-member who isn't a
        // contact gets the contact-request treatment (servers.md, open Q 6).
        if pin.contact.is_none() && !matches!(p.body, PayloadBody::Server { .. }) {
            if matches!(p.body, PayloadBody::Text { .. }) && self.admit_contact_request(peer) {
                if let Some(card) = pin.rows.first().map(co_member_card) {
                    self.events.emit(CoreEvent::ContactRequest { card });
                }
            }
            return Ok(MsgResponse::Rejected(RejectReason::NotAContact));
        }
        let meta = p.meta();
        if !meta_ok(&meta) {
            return Ok(MsgResponse::Rejected(RejectReason::Malformed));
        }
        match p.body {
            PayloadBody::Server {
                server_id,
                kind,
                data,
            } => {
                if !pin.server_ok(&server_id) {
                    return Ok(MsgResponse::Rejected(RejectReason::NotAMember));
                }
                let Ok(data) = unb64(&data) else {
                    return Ok(MsgResponse::Rejected(RejectReason::Malformed));
                };
                if data.len() > MAX_SERVER_PAYLOAD_BYTES {
                    return Ok(MsgResponse::Rejected(RejectReason::Malformed));
                }
                self.emit_server(ServerInbound {
                    peer_id: pid,
                    server_id,
                    kind,
                    data,
                    id: p.id,
                    ts: p.ts,
                    meta,
                });
            }
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
                if self.store.insert_message_with_meta(&m, &meta)? {
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
                // Contacts and co-members only, so strangers never occupy
                // limiter state. Both share the same per-peer limits.
                if !self.is_known_peer(&peer) {
                    return KeysResponse::Refused(RejectReason::NotAContact);
                }
                if !self.lim().otk_claims.try_take(&peer, Instant::now()) {
                    return KeysResponse::Refused(RejectReason::RateLimited);
                }
                match self.issue_key(peer) {
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

    /// A fresh OTK if the requester (and everyone together) is under the
    /// outstanding caps, otherwise the reusable fallback key. Never blocks.
    fn issue_key(&self, peer: PeerId) -> Result<Curve25519PublicKey> {
        let now = Instant::now();
        let l = &self.limits;
        let mut lim = self.lim();
        let ttl = l.outstanding_otk_ttl;
        lim.outstanding_otks.retain(|_, v| {
            v.retain(|(_, t)| now.saturating_duration_since(*t) < ttl);
            !v.is_empty()
        });
        let mine = lim.outstanding_otks.get(&peer).map_or(0, Vec::len);
        let total: usize = lim.outstanding_otks.values().map(Vec::len).sum();
        if mine >= l.max_outstanding_otks_per_peer || total >= l.max_outstanding_otks_total {
            return self.crypto.fallback_key();
        }
        let k = self
            .crypto
            .take_one_time_key(l.otk_pool_low_water, l.otk_pool_batch)?;
        lim.outstanding_otks.entry(peer).or_default().push((k, now));
        Ok(k)
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
        contact: &Pin,
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
                    // Re-read so pending/rejected state is current.
                    if let Some(pin) = self.pin(&contact.peer_id)? {
                        self.on_key_mismatch(&pin, &identity_key, None, KeyChangeSource::KeyClaim);
                    }
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
        contact: &Pin,
        p: &Payload,
    ) -> Result<(), DeliverError> {
        let peer = parse_peer(&contact.peer_id)?;
        self.greet(net, peer).await?;
        let pt = encode_payload(p).map_err(anyhow::Error::from)?;
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
        if self.pin(&pid)?.is_none() {
            return Ok(());
        }
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
                // Re-read every time: key changes / removals apply mid-flush.
                let Some(pin) = self.pin(&pid)? else {
                    return Ok(());
                };
                let p = match &row.item {
                    OutboxItem::Dm => {
                        let Some(m) = self.store.get_message(&row.msg_id)? else {
                            self.store.outbox_remove(&row.msg_id)?;
                            continue;
                        };
                        if pin.contact.is_none() {
                            continue; // only contacts get DMs
                        }
                        Payload::new(
                            pid.clone(),
                            m.id,
                            m.timestamp_ms,
                            PayloadBody::Text { text: m.body },
                            row.meta.clone(),
                        )
                    }
                    OutboxItem::Server { server_id, payload } => {
                        if !pin.rows.iter().any(|r| &r.server_id == server_id) {
                            self.store.outbox_remove(&row.msg_id)?; // no longer shared
                            continue;
                        }
                        if !pin.server_ok(server_id) {
                            return Ok(()); // key change pending: hold until resolved
                        }
                        match serde_json::from_slice::<Payload>(payload) {
                            Ok(p) => p,
                            Err(_) => {
                                self.store.outbox_remove(&row.msg_id)?;
                                continue;
                            }
                        }
                    }
                };
                self.store.outbox_attempt(&row.msg_id, now_ms())?;
                match self.deliver(&net, &pin, &p).await {
                    Ok(()) => {
                        self.store.outbox_remove(&row.msg_id)?;
                        if row.item == OutboxItem::Dm {
                            self.store
                                .set_message_status(&row.msg_id, DeliveryStatus::Delivered)?;
                            self.events.emit(CoreEvent::MessageStatus {
                                id: row.msg_id,
                                peer_id: pid.clone(),
                                status: DeliveryStatus::Delivered,
                            });
                        }
                    }
                    Err(DeliverError::NotAContact) => return Ok(()),
                    Err(DeliverError::Retry(e)) => return Err(e),
                }
            }
        }
    }

    /// Queue a server-scoped payload for co-member `peer_id` of `server_id`.
    /// Same outbox, ordering, retries and 7-day expiry as DMs; sent from the
    /// identity the membership uses. `meta` sets the optional envelope
    /// fields. Returns the payload id.
    pub fn queue_server_payload(
        self: &Arc<Self>,
        peer_id: &str,
        server_id: &str,
        kind: ServerPayloadKind,
        data: &[u8],
        meta: EnvelopeMeta,
    ) -> Result<String> {
        let peer = parse_peer(peer_id)?;
        let membership = self
            .server_membership(server_id)?
            .ok_or_else(|| anyhow!("not a member of {server_id}"))?;
        let pin = self
            .pin(peer_id)?
            .filter(|p| p.rows.iter().any(|r| r.server_id == server_id))
            .ok_or_else(|| anyhow!("{peer_id} is not a co-member of {server_id}"))?;
        if !pin.server_ok(server_id) {
            bail!("key change pending or unsigned card for {peer_id}; resolve it first");
        }
        if data.len() > MAX_SERVER_PAYLOAD_BYTES {
            bail!("server payload too large (max {MAX_SERVER_PAYLOAD_BYTES} bytes)");
        }
        if !meta_ok(&meta) {
            bail!("invalid envelope fields");
        }
        let now = now_ms();
        let p = Payload::new(
            peer_id.to_string(),
            uuid::Uuid::new_v4().to_string(),
            now,
            PayloadBody::Server {
                server_id: server_id.to_string(),
                kind,
                data: b64(data),
            },
            meta.clone(),
        );
        self.store.outbox_add_item(
            &p.id,
            peer_id,
            now,
            &OutboxItem::Server {
                server_id: server_id.to_string(),
                payload: serde_json::to_vec(&p)?,
            },
            &meta,
            membership.identity.as_str(),
        )?;
        self.spawn_flush(peer);
        Ok(p.id)
    }
}

/// Sanity limits on the optional envelope fields.
fn meta_ok(m: &EnvelopeMeta) -> bool {
    m.prev
        .as_ref()
        .is_none_or(|p| p.len() <= MAX_PREV_HASH_CHARS)
        && m.seq.is_none_or(|s| s <= i64::MAX as u64)
}

/// The card we hold for a co-member (for a `ContactRequest`).
fn co_member_card(r: &CoMemberRow) -> ContactCard {
    ContactCard {
        peer_id: r.peer_id.clone(),
        identity_key: r.identity_key.clone(),
        ed25519_key: r.ed25519_key.clone(),
        addrs: r.addrs.clone(),
        display_name: (!r.display_name.is_empty()).then(|| r.display_name.clone()),
        sig: r.card_sig.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::limits::*;

    pub(crate) fn open(
        key: u8,
    ) -> (
        Arc<Core>,
        tokio::sync::mpsc::UnboundedReceiver<CoreEvent>,
        tempfile::TempDir,
    ) {
        let dir = tempfile::tempdir().unwrap();
        let hub = EventHub::new();
        let rx = hub.subscribe();
        let core = Core::open(dir.path(), &[key; 32], CoreConfig::default(), hub).unwrap();
        (core, rx, dir)
    }

    fn open_lim(
        key: u8,
        limits: Limits,
    ) -> (
        Arc<Core>,
        tokio::sync::mpsc::UnboundedReceiver<CoreEvent>,
        tempfile::TempDir,
    ) {
        let dir = tempfile::tempdir().unwrap();
        let hub = EventHub::new();
        let rx = hub.subscribe();
        let core =
            Core::open_with_limits(dir.path(), &[key; 32], CoreConfig::default(), hub, limits)
                .unwrap();
        (core, rx, dir)
    }

    fn requests(evs: &[CoreEvent]) -> usize {
        evs.iter()
            .filter(|e| matches!(e, CoreEvent::ContactRequest { .. }))
            .count()
    }

    fn hello(card: &ContactCard) -> MsgRequest {
        MsgRequest::Hello { card: card.clone() }
    }

    /// `from` claims a key from `to`, opens an Olm session on it and
    /// delivers a text; returns the claimed key.
    fn claim_and_send(to: &Arc<Core>, from: &Arc<Core>, text: &str) -> Curve25519PublicKey {
        let pf = from.peer_id();
        let KeysResponse::Key {
            identity_key,
            one_time_key,
        } = to.handle_keys(pf, KeysRequest::ClaimOneTimeKey)
        else {
            panic!("claim refused")
        };
        let ik = Curve25519PublicKey::from_base64(&identity_key).unwrap();
        let otk = Curve25519PublicKey::from_base64(&one_time_key).unwrap();
        send_on(to, from, ik, otk, text);
        otk
    }

    fn send_on(
        to: &Arc<Core>,
        from: &Arc<Core>,
        ik: Curve25519PublicKey,
        otk: Curve25519PublicKey,
        text: &str,
    ) {
        let (pt, pf) = (to.peer_id(), from.peer_id());
        let pts = pt.to_base58();
        from.crypto.forget_sessions(&pts).unwrap();
        from.crypto.create_outbound(&pts, ik, otk).unwrap();
        let p = Payload {
            v: 1,
            to: pts.clone(),
            id: uuid::Uuid::new_v4().to_string(),
            ts: now_ms(),
            body: PayloadBody::Text { text: text.into() },

            seq: None,
            prev: None,
            expires_at: None,
        };
        let (t, c) = from
            .crypto
            .encrypt(&pts, &serde_json::to_vec(&p).unwrap())
            .unwrap();
        assert_eq!(
            to.handle_msg(pf, MsgRequest::Olm { t, c }),
            MsgResponse::Delivered
        );
    }

    async fn befriend(a: &Arc<Core>, b: &Arc<Core>) {
        a.add_contact(b.self_card(), false).await.unwrap();
        b.add_contact(a.self_card(), false).await.unwrap();
    }

    #[tokio::test]
    async fn card_v2_signatures_are_verified_and_never_downgraded() {
        let (a, mut rx, _da) = open_lim(
            1,
            Limits {
                hello_burst: 100,
                ..Limits::default()
            },
        );
        let (b, _rb, _db) = open(2);
        let pb = b.peer_id();
        let card_b = b.self_card();
        assert!(card_b.sig.is_some(), "own card is signed");
        let ik2 = vodozemac::olm::Account::new().curve25519_key();

        // Import: a card whose ik was swapped under B's signature is rejected
        // outright (no contact, no KeyChanged).
        let swapped = ContactCard {
            identity_key: ik2.to_base64(),
            ..card_b.clone()
        };
        assert!(a.add_contact(swapped.clone(), false).await.is_err());
        assert!(a.list_contacts().unwrap().is_empty());

        // Legacy (v1) contact: added unsigned, upgraded by B's signed Hello.
        let v1 = ContactCard {
            sig: None,
            ..card_b.clone()
        };
        let c = a.add_contact(v1.clone(), false).await.unwrap();
        assert!(!c.signed_card);
        assert_eq!(a.handle_msg(pb, hello(&card_b)), MsgResponse::Ok);
        assert!(a.list_contacts().unwrap()[0].signed_card);

        // Handshake: a forged signature is Malformed and raises nothing.
        assert_eq!(
            a.handle_msg(pb, hello(&swapped)),
            MsgResponse::Rejected(RejectReason::Malformed)
        );
        // No downgrade back to unsigned cards, by Hello or by import.
        assert_eq!(
            a.handle_msg(pb, hello(&v1)),
            MsgResponse::Rejected(RejectReason::Malformed)
        );
        assert!(a.add_contact(v1, false).await.is_err());
        assert!(key_changes(&drain(&mut rx)).is_empty());
        assert!(!a.list_contacts().unwrap()[0].key_changed);

        // A validly signed new key is a KeyChanged; accepting it keeps the
        // new signature, so the contact stays signed.
        let mut rotated = ContactCard {
            identity_key: ik2.to_base64(),
            ..card_b.clone()
        };
        qr::sign_card(&b.id.keypair, &mut rotated).unwrap();
        assert_eq!(
            a.handle_msg(pb, hello(&rotated)),
            MsgResponse::Rejected(RejectReason::KeyMismatch)
        );
        assert_eq!(key_changes(&drain(&mut rx)).len(), 1);
        let c = a.accept_key_change(&pb.to_base58()).unwrap();
        assert_eq!(c.identity_key, ik2.to_base64());
        assert!(c.signed_card && !c.verified);
        assert_eq!(a.handle_msg(pb, hello(&rotated)), MsgResponse::Ok);
    }

    /// `from` opens an Olm session to `to` (claims a key from it).
    fn open_session(to: &Arc<Core>, from: &Arc<Core>) -> KeysResponse {
        let r = to.handle_keys(from.peer_id(), KeysRequest::ClaimOneTimeKey);
        if let KeysResponse::Key {
            identity_key,
            one_time_key,
        } = &r
        {
            let pts = to.peer_id().to_base58();
            from.crypto.forget_sessions(&pts).unwrap();
            from.crypto
                .create_outbound(
                    &pts,
                    Curve25519PublicKey::from_base64(identity_key).unwrap(),
                    Curve25519PublicKey::from_base64(one_time_key).unwrap(),
                )
                .unwrap();
        }
        r
    }

    /// Encrypt `body` from `from` to `to` on the existing session.
    fn send_body(to: &Arc<Core>, from: &Arc<Core>, body: PayloadBody) -> MsgResponse {
        let pts = to.peer_id().to_base58();
        let p = Payload {
            v: 1,
            to: pts.clone(),
            id: uuid::Uuid::new_v4().to_string(),
            ts: now_ms(),
            body,

            seq: None,
            prev: None,
            expires_at: None,
        };
        let (t, c) = from
            .crypto
            .encrypt(&pts, &serde_json::to_vec(&p).unwrap())
            .unwrap();
        to.handle_msg(from.peer_id(), MsgRequest::Olm { t, c })
    }

    fn server_body(server_id: &str, data: &[u8]) -> PayloadBody {
        PayloadBody::Server {
            server_id: server_id.into(),
            kind: ServerPayloadKind::ServerOp,
            data: b64(data),
        }
    }

    fn rotated_card(core: &Arc<Core>) -> ContactCard {
        let mut c = ContactCard {
            identity_key: vodozemac::olm::Account::new().curve25519_key().to_base64(),
            ..core.self_card()
        };
        qr::sign_card(&core.id.keypair, &mut c).unwrap();
        c
    }

    #[tokio::test]
    async fn co_members_get_sessions_but_are_not_contacts() {
        let (a, mut rx, _da) = open(1);
        let mut srv = a.subscribe_server();
        let (b, _rb, _db) = open(2);
        let (c, _rc, _dc) = open(3);
        let pb = b.peer_id();

        // Membership is explicit about which identity it uses.
        assert!(a
            .add_server_membership("srv1", ServerIdentity::Dedicated, None)
            .is_err());
        let m = a
            .add_server_membership("srv1", ServerIdentity::Default, None)
            .unwrap();
        assert_eq!(
            (m.identity, m.history_visibility),
            (ServerIdentity::Default, None)
        );
        a.set_server_history("srv1", Some(HistoryVisibility::Shared))
            .unwrap();
        assert_eq!(
            a.server_membership("srv1")
                .unwrap()
                .unwrap()
                .history_visibility,
            Some(HistoryVisibility::Shared)
        );
        assert!(a
            .add_server_membership("bad id!", ServerIdentity::Default, None)
            .is_err());

        // Co-members need a membership and a signed card.
        assert!(a.add_co_member("srv2", b.self_card()).is_err());
        let unsigned = ContactCard {
            sig: None,
            ..b.self_card()
        };
        assert!(a.add_co_member("srv1", unsigned).is_err());
        let m = a.add_co_member("srv1", b.self_card()).unwrap();
        assert!(m.signed_card && !m.is_contact && !m.key_changed);
        assert!(a.list_contacts().unwrap().is_empty(), "never a contact");

        // Hello completes without a ContactRequest; key claims are served.
        assert_eq!(a.handle_msg(pb, hello(&b.self_card())), MsgResponse::Ok);
        assert!(matches!(open_session(&a, &b), KeysResponse::Key { .. }));

        // Server payloads for our shared server are delivered, bound to the
        // Olm sender; other servers are refused.
        assert_eq!(
            send_body(&a, &b, server_body("srv1", b"op")),
            MsgResponse::Delivered
        );
        let got = srv.try_recv().unwrap();
        assert_eq!(
            (
                got.peer_id.as_str(),
                got.server_id.as_str(),
                got.data.as_slice()
            ),
            (pb.to_base58().as_str(), "srv1", &b"op"[..])
        );
        assert_eq!(
            send_body(&a, &b, server_body("srv2", b"op")),
            MsgResponse::Rejected(RejectReason::NotAMember)
        );

        // DMs and calls from a co-member who isn't a contact are refused; a
        // DM is surfaced as one contact request, nothing is stored.
        let text = || PayloadBody::Text { text: "hi".into() };
        assert_eq!(
            send_body(&a, &b, text()),
            MsgResponse::Rejected(RejectReason::NotAContact)
        );
        assert_eq!(
            send_body(&a, &b, text()),
            MsgResponse::Rejected(RejectReason::NotAContact)
        );
        let signal = PayloadBody::Signal {
            call_id: "c".into(),
            kind: SignalKind::Ringing,
            payload: String::new(),
        };
        assert_eq!(
            send_body(&a, &b, signal),
            MsgResponse::Rejected(RejectReason::NotAContact)
        );
        assert_eq!(requests(&drain(&mut rx)), 1);
        assert!(a
            .list_messages(&pb.to_base58(), None, 10)
            .unwrap()
            .is_empty());
        assert!(a.list_contacts().unwrap().is_empty());

        // Same per-peer limits as contacts: OTK claims are throttled.
        let limited = (0..OTK_CLAIM_BURST)
            .map(|_| a.handle_keys(pb, KeysRequest::ClaimOneTimeKey))
            .filter(|r| *r == KeysResponse::Refused(RejectReason::RateLimited))
            .count();
        assert_eq!(limited, 1);

        // Strangers are unaffected: still a contact request, no keys.
        assert_eq!(
            a.handle_keys(c.peer_id(), KeysRequest::ClaimOneTimeKey),
            KeysResponse::Refused(RejectReason::NotAContact)
        );

        // Removing the record makes B a stranger again.
        a.remove_co_member("srv1", &pb.to_base58()).unwrap();
        assert_eq!(
            a.handle_msg(pb, hello(&b.self_card())),
            MsgResponse::Rejected(RejectReason::NotAContact)
        );
    }

    #[tokio::test]
    async fn co_member_key_pins_carry_over_and_block_server_traffic() {
        let (a, mut rx, _da) = open_lim(
            1,
            Limits {
                hello_burst: 100,
                ..Limits::default()
            },
        );
        let mut srv = a.subscribe_server();
        let (b, _rb, _db) = open(2);
        let pb = b.peer_id();
        let pbs = pb.to_base58();
        for s in ["srv1", "srv2"] {
            a.add_server_membership(s, ServerIdentity::Default, None)
                .unwrap();
        }
        a.add_co_member("srv1", b.self_card()).unwrap();
        assert!(matches!(open_session(&a, &b), KeysResponse::Key { .. }));

        // Another server's roster carries a different key for B: refused,
        // alert raised, and server traffic with B stops in every server.
        let rotated = rotated_card(&b);
        assert!(a.add_co_member("srv2", rotated.clone()).is_err());
        let evs = drain(&mut rx);
        let kc = key_changes(&evs);
        assert_eq!(kc.len(), 1);
        assert!(matches!(
            kc[0],
            CoreEvent::KeyChanged {
                source: KeyChangeSource::Roster,
                ..
            }
        ));
        assert!(a.co_member("srv1", &pbs).unwrap().unwrap().key_changed);
        assert_eq!(
            send_body(&a, &b, server_body("srv1", b"x")),
            MsgResponse::Rejected(RejectReason::NotAMember)
        );
        // The same key by Hello: refused, no second alert.
        assert_eq!(
            a.handle_msg(pb, hello(&rotated)),
            MsgResponse::Rejected(RejectReason::KeyMismatch)
        );
        assert!(key_changes(&drain(&mut rx)).is_empty());

        // Reject: the old pin stays and traffic resumes.
        a.reject_key_change(&pbs).unwrap();
        assert_eq!(
            send_body(&a, &b, server_body("srv1", b"x")),
            MsgResponse::Delivered
        );
        assert!(srv.try_recv().is_ok());

        // B becomes a contact too (same key): one pin for both classes.
        a.add_contact(b.self_card(), false).await.unwrap();
        assert!(a.co_member("srv1", &pbs).unwrap().unwrap().is_contact);
        // A contact card with another key is checked against the same pin.
        let rotated2 = rotated_card(&b);
        assert_eq!(
            a.handle_msg(pb, hello(&rotated2)),
            MsgResponse::Rejected(RejectReason::KeyMismatch)
        );
        assert_eq!(key_changes(&drain(&mut rx)).len(), 1);
        let c = &a.list_contacts().unwrap()[0];
        assert!(c.key_changed);
        assert!(a.co_member("srv1", &pbs).unwrap().unwrap().key_changed);
        // DMs keep the old key (as before); server traffic is blocked.
        assert_eq!(
            send_body(&a, &b, server_body("srv1", b"x")),
            MsgResponse::Rejected(RejectReason::NotAMember)
        );
        assert_eq!(
            send_body(&a, &b, PayloadBody::Text { text: "dm".into() }),
            MsgResponse::Delivered
        );

        // Accept updates every record of B together; never re-trusted silently.
        let c = a.accept_key_change(&pbs).unwrap();
        assert_eq!(c.identity_key, rotated2.identity_key);
        assert!(!c.verified && c.signed_card);
        let m = a.co_member("srv1", &pbs).unwrap().unwrap();
        assert_eq!(m.identity_key, rotated2.identity_key);
        assert!(!m.key_changed && m.signed_card);
        // Removing the contact keeps the co-member record.
        a.remove_contact(&pbs).unwrap();
        assert!(!a.co_member("srv1", &pbs).unwrap().unwrap().is_contact);
    }

    #[test]
    fn envelope_fields_are_optional_and_forward_compatible() {
        let body = || PayloadBody::Text { text: "t".into() };
        // Unset: omitted from the wire (byte-identical to the old format).
        let plain = Payload::new("p".into(), "i".into(), 1, body(), EnvelopeMeta::default());
        let j = serde_json::to_string(&plain).unwrap();
        assert!(!j.contains("seq") && !j.contains("prev") && !j.contains("expires_at"));
        // Set: round-trips.
        let meta = EnvelopeMeta {
            seq: Some(u32::MAX as u64 + 1),
            prev: Some("aGFzaA".into()),
            expires_at: Some(123),
        };
        let full = Payload::new("p".into(), "i".into(), 1, body(), meta.clone());
        let j = serde_json::to_string(&full).unwrap();
        assert_eq!(serde_json::from_str::<Payload>(&j).unwrap().meta(), meta);
        // An older core's Payload (no such fields) still decodes it.
        #[derive(serde::Deserialize)]
        #[allow(dead_code)]
        struct OldPayload {
            v: u8,
            to: String,
            id: String,
            ts: i64,
            body: PayloadBody,
        }
        serde_json::from_str::<OldPayload>(&j).unwrap();
        // Old JSON (absent fields) and future unknown fields decode here.
        let old = r#"{"v":1,"to":"p","id":"i","ts":1,"body":{"Text":{"text":"t"}}}"#;
        assert_eq!(
            serde_json::from_str::<Payload>(old).unwrap().meta(),
            EnvelopeMeta::default()
        );
        let future = r#"{"v":1,"to":"p","id":"i","ts":1,"body":{"Text":{"text":"t"}},"seq":3,"new_field":{"x":1}}"#;
        assert_eq!(
            serde_json::from_str::<Payload>(future).unwrap().seq,
            Some(3)
        );
    }

    #[tokio::test]
    async fn inbound_envelope_fields_are_stored_and_passed_on() {
        let (a, _ra, _da) = open(1);
        let (b, _rb, _db) = open(2);
        befriend(&a, &b).await;
        assert!(matches!(open_session(&a, &b), KeysResponse::Key { .. }));
        let meta = EnvelopeMeta {
            seq: Some(42),
            prev: Some("cHJldg".into()),
            expires_at: Some(9_999),
        };
        let pts = a.peer_id().to_base58();
        let p = Payload::new(
            pts.clone(),
            "m1".into(),
            now_ms(),
            PayloadBody::Text { text: "hi".into() },
            meta.clone(),
        );
        let (t, c) = b
            .crypto
            .encrypt(&pts, &serde_json::to_vec(&p).unwrap())
            .unwrap();
        assert_eq!(
            a.handle_msg(b.peer_id(), MsgRequest::Olm { t, c }),
            MsgResponse::Delivered
        );
        assert_eq!(a.store.message_meta("m1").unwrap(), Some(meta.clone()));

        // Oversized prev hash is refused.
        let bad = Payload::new(
            pts.clone(),
            "m2".into(),
            now_ms(),
            PayloadBody::Text { text: "hi".into() },
            EnvelopeMeta {
                prev: Some("x".repeat(MAX_PREV_HASH_CHARS + 1)),
                ..meta
            },
        );
        let (t, c) = b
            .crypto
            .encrypt(&pts, &serde_json::to_vec(&bad).unwrap())
            .unwrap();
        assert_eq!(
            a.handle_msg(b.peer_id(), MsgRequest::Olm { t, c }),
            MsgResponse::Rejected(RejectReason::Malformed)
        );
    }

    #[tokio::test]
    async fn outbox_queues_typed_server_payloads() {
        let (a, mut rx, _da) = open(1);
        let (b, _rb, _db) = open(2);
        let pbs = b.peer_id().to_base58();
        a.add_server_membership("srv1", ServerIdentity::Default, None)
            .unwrap();
        a.add_server_membership("srv2", ServerIdentity::Default, None)
            .unwrap();
        let q = |srv: &str| {
            a.queue_server_payload(
                &pbs,
                srv,
                ServerPayloadKind::ServerOp,
                b"op",
                EnvelopeMeta::default(),
            )
        };
        // Only to co-members of that server.
        assert!(q("srv1").is_err());
        a.add_co_member("srv1", b.self_card()).unwrap();
        assert!(q("srv2").is_err());
        let big = vec![0u8; MAX_SERVER_PAYLOAD_BYTES + 1];
        assert!(a
            .queue_server_payload(
                &pbs,
                "srv1",
                ServerPayloadKind::ServerOp,
                &big,
                EnvelopeMeta::default()
            )
            .is_err());
        let meta = EnvelopeMeta {
            seq: Some(1),
            prev: None,
            expires_at: Some(5),
        };
        let id = a
            .queue_server_payload(
                &pbs,
                "srv1",
                ServerPayloadKind::ServerKey,
                b"k",
                meta.clone(),
            )
            .unwrap();
        let rows = a.store.outbox_for(&pbs).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].msg_id, id);
        assert_eq!(rows[0].meta, meta);
        assert_eq!(rows[0].local_identity, "default");
        let OutboxItem::Server { server_id, payload } = &rows[0].item else {
            panic!("not a server row")
        };
        assert_eq!(server_id, "srv1");
        let p: Payload = serde_json::from_slice(payload).unwrap();
        assert_eq!((p.id.as_str(), p.meta()), (id.as_str(), meta));
        assert!(matches!(
            p.body,
            PayloadBody::Server {
                kind: ServerPayloadKind::ServerKey,
                ..
            }
        ));

        // Pending key change: queueing refused, row held.
        assert!(a.add_co_member("srv2", rotated_card(&b)).is_err());
        assert!(q("srv1").is_err());
        a.reject_key_change(&pbs).unwrap();

        // Same 7-day expiry as DMs, but no DM status event for server rows.
        drain(&mut rx);
        a.store
            .conn_for_test()
            .execute("UPDATE outbox SET created_at = 0", [])
            .unwrap();
        a.on_tick();
        assert!(a.store.outbox_for(&pbs).unwrap().is_empty());
        assert!(!drain(&mut rx)
            .iter()
            .any(|e| matches!(e, CoreEvent::MessageStatus { .. })));

        // Removing the co-member drops its queued server payloads.
        q("srv1").unwrap();
        a.remove_co_member("srv1", &pbs).unwrap();
        assert!(a.store.outbox_for(&pbs).unwrap().is_empty());
    }

    #[tokio::test]
    async fn accept_and_busy_signals_round_trip() {
        // Wire names must match calls/lib/src/signaling.dart.
        for (kind, wire) in [(SignalKind::Accept, "Accept"), (SignalKind::Busy, "Busy")] {
            let json = serde_json::to_string(&kind).unwrap();
            assert_eq!(json, format!("\"{wire}\""));
            assert_eq!(serde_json::from_str::<SignalKind>(&json).unwrap(), kind);
        }

        // Encrypted end to end through the inbound Olm path.
        let (a, mut rx, _da) = open(1);
        let (b, _rb, _db) = open(2);
        befriend(&a, &b).await;
        claim_and_send(&a, &b, "session");
        drain(&mut rx);
        let pa = a.peer_id().to_base58();
        for kind in [SignalKind::Accept, SignalKind::Busy] {
            let p = Payload {
                v: 1,
                to: pa.clone(),
                id: uuid::Uuid::new_v4().to_string(),
                ts: now_ms(),
                body: PayloadBody::Signal {
                    call_id: "c1".into(),
                    kind,
                    payload: "busy".into(),
                },

                seq: None,
                prev: None,
                expires_at: None,
            };
            let (t, c) = b
                .crypto
                .encrypt(&pa, &serde_json::to_vec(&p).unwrap())
                .unwrap();
            assert_eq!(
                a.handle_msg(b.peer_id(), MsgRequest::Olm { t, c }),
                MsgResponse::Delivered
            );
            let evs = drain(&mut rx);
            assert!(
                evs.iter()
                    .any(|e| matches!(e, CoreEvent::Signal { kind: k, call_id, .. }
                    if *k == kind && call_id == "c1")),
                "{kind:?} not delivered: {evs:?}"
            );
        }
    }

    #[tokio::test]
    async fn hello_flood_is_deduped_and_throttled() {
        let (a, mut rx, _da) = open_lim(
            1,
            Limits {
                contact_request_burst: 3,
                ..Limits::default()
            },
        );
        // One unknown identity hammering: a single ContactRequest, then RateLimited.
        let (s, _rs, _ds) = open(2);
        let card = s.self_card();
        let resp: Vec<_> = (0..20)
            .map(|_| a.handle_msg(s.peer_id(), hello(&card)))
            .collect();
        assert_eq!(requests(&drain(&mut rx)), 1);
        let limited = resp
            .iter()
            .filter(|r| **r == MsgResponse::Rejected(RejectReason::RateLimited))
            .count();
        assert_eq!(limited, 20 - HELLO_BURST as usize);

        // Many distinct unknown identities: only the global budget gets through.
        let mut keep = vec![];
        for k in 10..16 {
            let (c, _r, d) = open(k);
            assert_eq!(
                a.handle_msg(c.peer_id(), hello(&c.self_card())),
                MsgResponse::Rejected(RejectReason::NotAContact)
            );
            keep.push((c, d));
        }
        assert_eq!(requests(&drain(&mut rx)), 2, "3 budget - 1 already used");

        // A known contact is unaffected by the flood.
        let (b, _rb, _db) = open(3);
        befriend(&a, &b).await;
        assert_eq!(
            a.handle_msg(b.peer_id(), hello(&b.self_card())),
            MsgResponse::Ok
        );
        claim_and_send(&a, &b, "still here");
    }

    #[tokio::test]
    async fn pending_contact_requests_are_capped() {
        let (a, mut rx, _da) = open_lim(
            1,
            Limits {
                contact_request_burst: 100,
                max_pending_contact_requests: 2,
                ..Limits::default()
            },
        );
        let peers: Vec<_> = (10..14).map(open).collect();
        for (c, _, _) in &peers {
            a.handle_msg(c.peer_id(), hello(&c.self_card()));
        }
        assert_eq!(requests(&drain(&mut rx)), 2);
        // Accepting one frees a slot.
        a.add_contact(peers[0].0.self_card(), false).await.unwrap();
        a.handle_msg(peers[3].0.peer_id(), hello(&peers[3].0.self_card()));
        assert_eq!(requests(&drain(&mut rx)), 1);
    }

    #[tokio::test]
    async fn otk_claim_flood_is_rationed_and_others_still_get_sessions() {
        let (a, _ra, _da) = open(1);
        let (b, _rb, _db) = open(2);
        let (c, _rc, _dc) = open(3);
        befriend(&a, &b).await;
        befriend(&a, &c).await;
        let fallback = a.crypto.fallback_key().unwrap();
        let claim = |p: &Arc<Core>| match a.handle_keys(p.peer_id(), KeysRequest::ClaimOneTimeKey) {
            KeysResponse::Key { one_time_key, .. } => {
                Some(Curve25519PublicKey::from_base64(&one_time_key).unwrap())
            }
            KeysResponse::Refused(r) => {
                assert_eq!(r, RejectReason::RateLimited);
                None
            }
        };

        // B floods: 2 fresh keys (outstanding cap), then the fallback key, then throttled.
        let got: Vec<_> = (0..10).map(|_| claim(&b)).collect();
        let fresh: Vec<_> = got.iter().flatten().filter(|k| **k != fallback).collect();
        assert_eq!(fresh.len(), MAX_OUTSTANDING_OTKS_PER_PEER);
        assert_ne!(fresh[0], fresh[1]);
        assert_eq!(got[2], Some(fallback));
        assert_eq!(got.iter().flatten().count(), OTK_CLAIM_BURST as usize);
        // The pool was replenished in one batch and only drained by what B got.
        assert_eq!(a.crypto.pool_size().unwrap(), OTK_POOL_BATCH - 2);

        // C is unaffected: gets a fresh key and a working session.
        let k = claim_and_send(&a, &c, "hi from c");
        assert_ne!(k, fallback);

        // Sessions on B's fresh and fallback keys work too, and using a fresh
        // key frees B's outstanding slot.
        let aik = a.id.curve25519;
        send_on(&a, &b, aik, *fresh[0], "fresh");
        assert_eq!(a.lim().outstanding_otks[&b.peer_id()].len(), 1);
        send_on(&a, &b, aik, fallback, "fallback");
    }

    #[tokio::test]
    async fn otk_total_cap_falls_back_without_blocking() {
        let (a, _ra, _da) = open_lim(
            1,
            Limits {
                max_outstanding_otks_total: 2,
                otk_pool_low_water: 1,
                otk_pool_batch: 1,
                ..Limits::default()
            },
        );
        let (b, _rb, _db) = open(2);
        let (c, _rc, _dc) = open(3);
        befriend(&a, &b).await;
        befriend(&a, &c).await;
        let fallback = a.crypto.fallback_key().unwrap();
        for _ in 0..2 {
            assert!(matches!(
                a.handle_keys(b.peer_id(), KeysRequest::ClaimOneTimeKey),
                KeysResponse::Key { .. }
            ));
        }
        // Pool exhausted for fresh keys: C gets the fallback key and still connects.
        assert_eq!(claim_and_send(&a, &c, "via fallback"), fallback);
    }

    fn drain(rx: &mut tokio::sync::mpsc::UnboundedReceiver<CoreEvent>) -> Vec<CoreEvent> {
        let mut v = vec![];
        while let Ok(e) = rx.try_recv() {
            v.push(e);
        }
        v
    }

    fn key_changes(evs: &[CoreEvent]) -> Vec<&CoreEvent> {
        evs.iter()
            .filter(|e| matches!(e, CoreEvent::KeyChanged { .. }))
            .collect()
    }

    #[tokio::test]
    async fn key_change_is_flagged_rejected_and_resolvable() {
        // Many Hellos from one peer here; keep them below the rate limit.
        let (a, mut rx, _da) = open_lim(
            1,
            Limits {
                hello_burst: 100,
                ..Limits::default()
            },
        );
        let (b, _rxb, _db) = open(2);
        let card_b = b.self_card();
        let pb = card_b.peer_id.clone();
        let peer_b: PeerId = pb.parse().unwrap();
        a.add_contact(card_b.clone(), true).await.unwrap();
        drain(&mut rx);

        // B (same device key / PeerId) shows up with a different Olm identity key.
        // B's device key signs a new Olm identity key (card v2): a validly
        // signed but changed key must still never be auto-trusted.
        let forged = |ik: Curve25519PublicKey| {
            let mut c = ContactCard {
                identity_key: ik.to_base64(),
                ..card_b.clone()
            };
            qr::sign_card(&b.id.keypair, &mut c).unwrap();
            c
        };
        let ik2 = vodozemac::olm::Account::new().curve25519_key();
        let r = a.handle_msg(peer_b, MsgRequest::Hello { card: forged(ik2) });
        assert_eq!(r, MsgResponse::Rejected(RejectReason::KeyMismatch));
        let evs = drain(&mut rx);
        let kc = key_changes(&evs);
        assert_eq!(kc.len(), 1);
        let CoreEvent::KeyChanged {
            peer_id,
            old_identity_key,
            new_identity_key,
            old_safety_number,
            new_safety_number,
            source,
        } = kc[0]
        else {
            unreachable!()
        };
        assert_eq!(peer_id, &pb);
        assert_eq!(old_identity_key, &card_b.identity_key);
        assert_eq!(new_identity_key, &ik2.to_base64());
        assert_ne!(old_safety_number, new_safety_number);
        assert_eq!(*source, KeyChangeSource::Hello);

        // Not trusted: old key kept, contact flagged, still verified=true until resolved.
        let c = &a.list_contacts().unwrap()[0];
        assert_eq!(c.identity_key, card_b.identity_key);
        assert!(c.key_changed);
        assert_eq!(
            c.pending_identity_key.as_deref(),
            Some(ik2.to_base64().as_str())
        );

        // Same key again (Hello or re-scanned card): still rejected, no duplicate alert.
        assert_eq!(
            a.handle_msg(peer_b, MsgRequest::Hello { card: forged(ik2) }),
            MsgResponse::Rejected(RejectReason::KeyMismatch)
        );
        assert!(a.add_contact(forged(ik2), false).await.is_err());
        assert!(key_changes(&drain(&mut rx)).is_empty());
        // Key claims are refused to nobody else: the old key still answers.
        assert_eq!(
            a.handle_msg(
                peer_b,
                MsgRequest::Hello {
                    card: card_b.clone()
                }
            ),
            MsgResponse::Ok
        );

        // Reject: flag cleared, the refused key never alerts again.
        a.reject_key_change(&pb).unwrap();
        assert!(!a.list_contacts().unwrap()[0].key_changed);
        assert_eq!(
            a.handle_msg(peer_b, MsgRequest::Hello { card: forged(ik2) }),
            MsgResponse::Rejected(RejectReason::KeyMismatch)
        );
        assert!(key_changes(&drain(&mut rx)).is_empty());
        assert!(a.reject_key_change(&pb).is_err(), "nothing pending");

        // A different new key alerts again (via a scanned card this time).
        let ik3 = vodozemac::olm::Account::new().curve25519_key();
        assert!(a.add_contact(forged(ik3), false).await.is_err());
        let evs = drain(&mut rx);
        let kc = key_changes(&evs);
        assert_eq!(kc.len(), 1);
        assert!(matches!(
            kc[0],
            CoreEvent::KeyChanged {
                source: KeyChangeSource::ContactCard,
                ..
            }
        ));

        // Accept: new key trusted, contact unverified, flag cleared.
        let c = a.accept_key_change(&pb).unwrap();
        assert_eq!(c.identity_key, ik3.to_base64());
        assert!(!c.verified && !c.key_changed && c.pending_identity_key.is_none());
        assert_eq!(
            a.handle_msg(peer_b, MsgRequest::Hello { card: forged(ik3) }),
            MsgResponse::Ok
        );
        assert_eq!(
            a.handle_msg(
                peer_b,
                MsgRequest::Hello {
                    card: card_b.clone()
                }
            ),
            MsgResponse::Rejected(RejectReason::KeyMismatch),
            "the old key is now the changed one"
        );
    }
}
