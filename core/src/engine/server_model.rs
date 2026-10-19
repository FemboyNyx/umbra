//! Community server model, Phase 1 (docs/servers.md, "Membership, roles and
//! moderation"): owner-signed manifests, admin-signed roster ops, channels,
//! roles and permissions, and authorized-only invites.
//!
//! Pure data and verification; no I/O. Everything signed is carried and
//! stored as the exact bytes that were signed (canonical CBOR as produced
//! by the signer), so nothing is ever re-encoded before verifying or hashing.

use std::collections::BTreeSet;

use anyhow::{anyhow, bail, Context, Result};
use hmac::{Hmac, Mac};
use libp2p::identity::{self, ed25519};
use libp2p::PeerId;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::api::types::ContactCard;
use crate::engine::qr;
use crate::engine::servers::HistoryVisibility;
use crate::engine::util::{b64, unb64};

// ---- limits and defaults (servers.md "Summary of decisions") ----

/// Roster cap (decided).
pub const MAX_MEMBERS: usize = 250;
/// Recovery keys: 1 to 3, required (decided).
pub const MAX_RECOVERY_KEYS: usize = 3;
/// Host ciphertext retention cap, days (decided). Policy can shorten it.
pub const MAX_RETENTION_DAYS: u32 = 30;
/// File size cap (decided).
pub const MAX_FILE_BYTES: u64 = 100 * 1024 * 1024;
/// A signed manifest, as bytes. 250 cards without addresses fit easily.
pub const MAX_MANIFEST_BYTES: usize = 256 * 1024;
pub const MAX_CHANNELS: usize = 200;
pub const MAX_NAME_CHARS: usize = 100;
pub const MAX_HOSTS: usize = 8;
/// Invite defaults (proposed): 7 days, one use.
pub const DEFAULT_INVITE_TTL_MS: i64 = 7 * 24 * 3600 * 1000;
pub const DEFAULT_INVITE_USES: u32 = 1;
pub const MAX_INVITE_USES: u32 = 1000;

const MANIFEST_CONTEXT: &[u8] = b"umbra-manifest-v1";
const OP_CONTEXT: &[u8] = b"umbra-roster-op-v1";
const INVITE_CONTEXT: &[u8] = b"umbra-invite-v1";
const JOIN_PROOF_CONTEXT: &[u8] = b"umbra-join-v1";

// ---- encoding helpers ----

pub fn cbor<T: Serialize>(v: &T) -> Result<Vec<u8>> {
    let mut out = Vec::new();
    ciborium::into_writer(v, &mut out).map_err(|e| anyhow!("cbor encode: {e}"))?;
    Ok(out)
}

pub fn uncbor<T: DeserializeOwned>(b: &[u8]) -> Result<T> {
    ciborium::from_reader(b).map_err(|e| anyhow!("cbor decode: {e}"))
}

/// Unpadded base64url (ServerId, ChannelId, hashes).
pub fn b64url(b: &[u8]) -> String {
    use base64::Engine;
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(b)
}

pub fn sha256_url(b: &[u8]) -> String {
    b64url(&Sha256::digest(b))
}

fn signed_message(ctx: &[u8], bytes: &[u8]) -> Vec<u8> {
    let mut m = ctx.to_vec();
    m.extend_from_slice(bytes);
    m
}

pub fn ed_public(b64key: &str) -> Result<ed25519::PublicKey> {
    ed25519::PublicKey::try_from_bytes(&unb64(b64key)?).context("invalid ed25519 key")
}

fn verify(key: &str, ctx: &[u8], bytes: &[u8], sig: &[u8]) -> Result<bool> {
    Ok(ed_public(key)?.verify(&signed_message(ctx, bytes), sig))
}

/// A dedicated Ed25519 key (owner key `K_S`, recovery keys).
pub struct ServerKey(ed25519::Keypair);

impl ServerKey {
    pub fn generate() -> Self {
        Self(ed25519::Keypair::generate())
    }
    pub fn from_secret(secret: &[u8]) -> Result<Self> {
        let mut s = secret.to_vec();
        let sk = ed25519::SecretKey::try_from_bytes(&mut s).context("invalid key")?;
        Ok(Self(ed25519::Keypair::from(sk)))
    }
    /// The 32-byte secret. Never crosses the FFI except as a recovery
    /// secret the user asked to export.
    pub fn secret(&self) -> zeroize::Zeroizing<Vec<u8>> {
        zeroize::Zeroizing::new(self.0.secret().as_ref().to_vec())
    }
    pub fn public_b64(&self) -> String {
        b64(&self.0.public().to_bytes())
    }
    fn sign(&self, ctx: &[u8], bytes: &[u8]) -> Vec<u8> {
        self.0.sign(&signed_message(ctx, bytes))
    }
}

/// Short fingerprint of an owner key, shown in join previews:
/// SHA-256 of the raw key, first 16 bytes, hex in groups of 4.
pub fn key_fingerprint(key_b64: &str) -> Result<String> {
    let raw = unb64(key_b64)?;
    let h = hex::encode(&Sha256::digest(&raw)[..16]);
    Ok(h.as_bytes()
        .chunks(4)
        .map(|c| std::str::from_utf8(c).unwrap_or(""))
        .collect::<Vec<_>>()
        .join(" "))
}

// ---- roles and permissions ----

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Role {
    Owner,
    Admin,
    Member,
}

/// What a role may do. The owner may do everything; the owner alone changes
/// roles, permissions (`policy`), recovery keys and hosts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Permission {
    /// Create invites and admit joiners who used them (roles in `policy.invite`).
    Invite,
    /// Create and edit channels (admins).
    ManageChannels,
    /// Kick members (admins).
    Kick,
    /// Ban and unban members (admins).
    Ban,
    /// Delete any message (admins).
    DeleteMessages,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChannelKind {
    Text,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Channel {
    /// 16 random bytes, base64url, so hosts never see names.
    pub id: String,
    pub name: String,
    pub kind: ChannelKind,
    /// Roles that can read / post. Owner always can.
    pub read: Vec<Role>,
    pub post: Vec<Role>,
    /// Overrides `policy.history` for this channel.
    pub history: Option<HistoryVisibility>,
    /// Disappearing timer, seconds (stored; Phase 1 PR 5 enforces).
    pub timer: Option<u64>,
}

impl Channel {
    pub fn new_text(name: &str) -> Self {
        Self {
            id: b64url(&rand_bytes::<16>()),
            name: name.to_string(),
            kind: ChannelKind::Text,
            read: vec![Role::Owner, Role::Admin, Role::Member],
            post: vec![Role::Owner, Role::Admin, Role::Member],
            history: None,
            timer: None,
        }
    }
}

pub fn rand_bytes<const N: usize>() -> [u8; N] {
    let mut out = [0u8; N];
    let mut i = 0;
    while i < N {
        let u = uuid::Uuid::new_v4();
        for b in u.as_bytes() {
            if i < N {
                out[i] = *b;
                i += 1;
            }
        }
    }
    out
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RosterEntry {
    /// Card v2 (signature required), without addresses.
    pub card: ContactCard,
    pub role: Role,
    pub added_by: String,
    pub added_seq: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Ban {
    pub peer_id: String,
    /// Manifest seq at (or op base_seq on top of) which the ban was made.
    pub seq: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HostEntry {
    pub peer_id: String,
    pub addrs: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Policy {
    /// Roles allowed to create invites. Default owner and admin (Nyx).
    pub invite: Vec<Role>,
    /// History for new members. Default shared (Nyx).
    pub history: HistoryVisibility,
    /// How far back shared history goes, seconds. None = everything kept.
    pub history_window: Option<u64>,
    /// Host ciphertext retention, days, at most 30.
    pub retention_days: u32,
    /// Max message lifetime per role, seconds (stored; PR 5 enforces).
    pub max_lifetime: Vec<(Role, u64)>,
    pub slow_mode_secs: Option<u32>,
    pub max_file_bytes: u64,
    pub previews: bool,
    /// No files or links for the first 24 h.
    pub new_member_limits: bool,
}

impl Default for Policy {
    fn default() -> Self {
        Self {
            invite: vec![Role::Owner, Role::Admin],
            history: HistoryVisibility::Shared,
            history_window: None,
            retention_days: MAX_RETENTION_DAYS,
            max_lifetime: Vec::new(),
            slow_mode_secs: None,
            max_file_bytes: MAX_FILE_BYTES,
            previews: true,
            new_member_limits: false,
        }
    }
}

/// `ServerManifest`, signed by the owner key `K_S` (or, for a takeover, by
/// a recovery key of the previous manifest).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Manifest {
    pub v: u8,
    /// `None` only in the genesis manifest (seq 0); the ServerId is its hash.
    pub server_id: Option<String>,
    pub seq: u64,
    /// Hash of the previous signed manifest bytes (base64url).
    pub prev: Option<String>,
    pub owner: String,
    pub recovery: Vec<String>,
    pub name: String,
    /// `FileRef` once files exist (PR 3).
    pub icon: Option<String>,
    pub channels: Vec<Channel>,
    pub roster: Vec<RosterEntry>,
    pub bans: Vec<Ban>,
    pub hosts: Vec<HostEntry>,
    /// Pack list `FileRef`s (Phase 2).
    pub packs: Vec<String>,
    pub policy: Policy,
    /// Signed by a recovery key of the previous manifest, naming a new owner.
    pub takeover: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SignedManifest {
    #[serde(with = "serde_bytes")]
    pub bytes: Vec<u8>,
    #[serde(with = "serde_bytes")]
    pub sig: Vec<u8>,
}

impl SignedManifest {
    pub fn sign(m: &Manifest, key: &ServerKey) -> Result<Self> {
        let bytes = cbor(m)?;
        if bytes.len() > MAX_MANIFEST_BYTES {
            bail!("manifest too large");
        }
        Ok(Self {
            sig: key.sign(MANIFEST_CONTEXT, &bytes),
            bytes,
        })
    }
    pub fn manifest(&self) -> Result<Manifest> {
        if self.bytes.len() > MAX_MANIFEST_BYTES {
            bail!("manifest too large");
        }
        uncbor(&self.bytes)
    }
    /// `EnvHash`-style hash the next manifest's `prev` points at.
    pub fn hash(&self) -> String {
        sha256_url(&self.bytes)
    }
    pub fn to_wire(&self) -> Result<Vec<u8>> {
        cbor(self)
    }
    pub fn from_wire(b: &[u8]) -> Result<Self> {
        if b.len() > MAX_MANIFEST_BYTES + 1024 {
            bail!("manifest too large");
        }
        uncbor(b)
    }
    fn signed_by(&self, key: &str) -> bool {
        verify(key, MANIFEST_CONTEXT, &self.bytes, &self.sig).unwrap_or(false)
    }
}

/// Card as kept in a roster: v2 signature required, addresses dropped.
pub fn roster_card(card: &ContactCard) -> Result<ContactCard> {
    if card.sig.is_none() {
        bail!("roster members need a signed (v2) card");
    }
    qr::validate_card(card)?;
    Ok(ContactCard {
        addrs: Vec::new(),
        ..card.clone()
    })
}

fn check_name(n: &str) -> Result<()> {
    if n.trim().is_empty() || n.chars().count() > MAX_NAME_CHARS {
        bail!("names must be 1 to {MAX_NAME_CHARS} characters");
    }
    Ok(())
}

/// Structural checks every manifest must pass.
pub fn validate_manifest(m: &Manifest) -> Result<()> {
    if m.v != 1 {
        bail!("unsupported manifest version {}", m.v);
    }
    check_name(&m.name)?;
    ed_public(&m.owner)?;
    if m.recovery.is_empty() || m.recovery.len() > MAX_RECOVERY_KEYS {
        bail!("a server needs 1 to {MAX_RECOVERY_KEYS} recovery keys");
    }
    for k in &m.recovery {
        ed_public(k)?;
        if *k == m.owner {
            bail!("a recovery key can't be the owner key");
        }
    }
    if m.roster.len() > MAX_MEMBERS {
        bail!("at most {MAX_MEMBERS} members");
    }
    if m.channels.len() > MAX_CHANNELS || m.hosts.len() > MAX_HOSTS {
        bail!("too many channels or hosts");
    }
    let mut seen = BTreeSet::new();
    let mut owners = 0;
    for e in &m.roster {
        if !e.card.addrs.is_empty() || e.card.sig.is_none() {
            bail!("roster cards are signed and carry no addresses");
        }
        qr::validate_card(&e.card)?;
        if !seen.insert(e.card.peer_id.clone()) {
            bail!("duplicate roster entry");
        }
        owners += usize::from(e.role == Role::Owner);
    }
    if owners != 1 {
        bail!("the roster needs exactly one owner device");
    }
    let mut ids = BTreeSet::new();
    for c in &m.channels {
        check_name(&c.name)?;
        if c.id.len() != 22 || !ids.insert(c.id.clone()) {
            bail!("bad or duplicate channel id");
        }
    }
    if m.policy.retention_days == 0 || m.policy.retention_days > MAX_RETENTION_DAYS {
        bail!("retention must be 1 to {MAX_RETENTION_DAYS} days");
    }
    if m.policy.max_file_bytes > MAX_FILE_BYTES {
        bail!("file size limit above 100 MiB");
    }
    if m.bans.iter().any(|b| seen.contains(&b.peer_id)) {
        bail!("a banned peer is in the roster");
    }
    Ok(())
}

/// Genesis: the ServerId is the hash of these bytes.
pub fn server_id_of_genesis(g: &SignedManifest) -> String {
    sha256_url(&g.bytes)
}

/// What happened when a manifest was offered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ManifestCheck {
    /// Newer and valid: adopt it.
    Accept,
    /// The same bytes we have.
    Duplicate,
    /// Lower `seq` than the highest seen (invariant S5): refused.
    Rollback { have: u64, got: u64 },
    /// Same `seq`, different bytes, valid signature: the owner key is
    /// compromised or misused. Freeze the server.
    Fork { seq: u64 },
}

/// Check `next` against the current manifest `cur` of `server_id`.
///
/// * `seq = cur.seq + 1`: `prev` must be `cur`'s hash and it must be signed
///   by `cur.owner`, or (a takeover) by one of `cur.recovery`.
/// * `seq > cur.seq + 1` (we missed some): only if signed by `cur.owner`,
///   so the chain of trust is unbroken; transfers and takeovers in a gap
///   need the intermediate manifests.
pub fn check_next(
    server_id: &str,
    cur: &SignedManifest,
    next: &SignedManifest,
) -> Result<ManifestCheck> {
    let c = cur.manifest()?;
    let n = next.manifest()?;
    validate_manifest(&n)?;
    let sid_ok = match n.server_id.as_deref() {
        Some(s) => s == server_id,
        None => n.seq == 0 && server_id_of_genesis(next) == server_id,
    };
    if !sid_ok {
        bail!("manifest is for another server");
    }
    if n.seq < c.seq {
        return Ok(ManifestCheck::Rollback {
            have: c.seq,
            got: n.seq,
        });
    }
    if n.seq == c.seq {
        if next.bytes == cur.bytes {
            return Ok(ManifestCheck::Duplicate);
        }
        // Two manifests for one seq, both validly signed by the key that
        // was entitled to sign that seq, is a fork.
        let ok = next.signed_by(&c.owner) || c.recovery.iter().any(|k| next.signed_by(k));
        if ok {
            return Ok(ManifestCheck::Fork { seq: n.seq });
        }
        bail!("bad manifest signature");
    }
    if n.seq == c.seq + 1 {
        if n.prev.as_deref() != Some(cur.hash().as_str()) {
            bail!("manifest prev does not match");
        }
        let by_owner = next.signed_by(&c.owner);
        let by_recovery = c.recovery.iter().any(|k| next.signed_by(k));
        if n.takeover {
            if !by_recovery {
                bail!("takeover not signed by a recovery key");
            }
        } else if !by_owner {
            bail!("manifest not signed by the owner key");
        }
        return Ok(ManifestCheck::Accept);
    }
    if !n.takeover && n.owner == c.owner && next.signed_by(&c.owner) {
        return Ok(ManifestCheck::Accept);
    }
    bail!("missing intermediate manifests")
}

/// First manifest a joiner sees: no chain, so it is anchored on the owner
/// key from the invite (shown as a fingerprint in the join preview).
pub fn check_first(server_id: &str, owner_key: &str, m: &SignedManifest) -> Result<()> {
    let man = m.manifest()?;
    validate_manifest(&man)?;
    let sid_ok = match man.server_id.as_deref() {
        Some(s) => s == server_id,
        None => server_id_of_genesis(m) == server_id,
    };
    if !sid_ok {
        bail!("manifest is for another server");
    }
    if man.owner != owner_key || !m.signed_by(owner_key) {
        bail!("manifest not signed by the invite's owner key");
    }
    Ok(())
}

/// Genesis manifest check (founding members, tests).
pub fn check_genesis(g: &SignedManifest) -> Result<String> {
    let m = g.manifest()?;
    validate_manifest(&m)?;
    if m.seq != 0 || m.server_id.is_some() || m.prev.is_some() || !g.signed_by(&m.owner) {
        bail!("not a valid genesis manifest");
    }
    Ok(server_id_of_genesis(g))
}

// ---- roster ops ----

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OpKind {
    /// Admit a joiner who used `invite`, an invite the op signer created.
    Admit {
        card: ContactCard,
        invite: SignedInvite,
    },
    Kick {
        peer_id: String,
    },
    Ban {
        peer_id: String,
    },
    Unban {
        peer_id: String,
    },
    DeleteMessage {
        channel_id: String,
        message_id: String,
    },
    CreateChannel {
        channel: Channel,
    },
    EditChannel {
        channel: Channel,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RosterOp {
    pub server_id: String,
    pub base_seq: u64,
    pub op: OpKind,
    pub ts: i64,
    /// Signer's PeerId; its device key signs (and derives it).
    pub signer: String,
    pub signer_ed: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SignedOp {
    #[serde(with = "serde_bytes")]
    pub bytes: Vec<u8>,
    #[serde(with = "serde_bytes")]
    pub sig: Vec<u8>,
}

impl SignedOp {
    pub fn sign(op: &RosterOp, device: &identity::Keypair) -> Result<Self> {
        let bytes = cbor(op)?;
        let sig = device
            .sign(&signed_message(OP_CONTEXT, &bytes))
            .map_err(|e| anyhow!("sign op: {e}"))?;
        Ok(Self { bytes, sig })
    }
    /// Decode and check the signature and signer binding (not permissions).
    pub fn verified(&self) -> Result<RosterOp> {
        if self.bytes.len() > 64 * 1024 {
            bail!("op too large");
        }
        let op: RosterOp = uncbor(&self.bytes)?;
        let key = ed_public(&op.signer_ed)?;
        let peer = PeerId::from_public_key(&identity::PublicKey::from(key.clone()));
        if peer.to_base58() != op.signer {
            bail!("op signer does not match its key");
        }
        if !key.verify(&signed_message(OP_CONTEXT, &self.bytes), &self.sig) {
            bail!("bad op signature");
        }
        Ok(op)
    }
    pub fn hash(&self) -> String {
        sha256_url(&self.bytes)
    }
    pub fn to_wire(&self) -> Result<Vec<u8>> {
        cbor(self)
    }
    pub fn from_wire(b: &[u8]) -> Result<Self> {
        uncbor(b)
    }
}

// ---- invites ----

/// The signed part of an invite (servers.md "Invites and joining"). The
/// secret itself is not in here, only its hash, so an `Admit` op can carry
/// the signed invite to other members without leaking the secret.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Invite {
    pub server_id: String,
    /// Current owner key (the joiner's trust anchor; fingerprint shown).
    pub owner_key: String,
    pub server_name: String,
    pub inviter: ContactCard,
    pub invite_id: String,
    /// SHA-256 of the 32-byte invite secret.
    pub secret_hash: String,
    pub expires_at: i64,
    pub max_uses: u32,
    pub needs_approval: bool,
    pub hosts: Vec<HostEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SignedInvite {
    #[serde(with = "serde_bytes")]
    pub bytes: Vec<u8>,
    #[serde(with = "serde_bytes")]
    pub sig: Vec<u8>,
}

impl SignedInvite {
    pub fn sign(inv: &Invite, device: &identity::Keypair) -> Result<Self> {
        let bytes = cbor(inv)?;
        let sig = device
            .sign(&signed_message(INVITE_CONTEXT, &bytes))
            .map_err(|e| anyhow!("sign invite: {e}"))?;
        Ok(Self { bytes, sig })
    }
    /// Decode, and check the inviter's card and signature.
    pub fn verified(&self) -> Result<Invite> {
        if self.bytes.len() > 16 * 1024 {
            bail!("invite too large");
        }
        let inv: Invite = uncbor(&self.bytes)?;
        if inv.inviter.sig.is_none() {
            bail!("inviter card is not signed");
        }
        qr::validate_card(&inv.inviter)?;
        if !verify(
            &inv.inviter.ed25519_key,
            INVITE_CONTEXT,
            &self.bytes,
            &self.sig,
        )? {
            bail!("bad invite signature");
        }
        crate::engine::servers::validate_server_id(&inv.server_id)?;
        Ok(inv)
    }
}

/// `umbra://join?i=<signed invite, base64url CBOR>&s=<secret, base64url>`.
pub fn build_join_link(inv: &SignedInvite, secret: &[u8; 32]) -> Result<String> {
    Ok(format!(
        "umbra://join?i={}&s={}",
        b64url(&cbor(inv)?),
        b64url(secret)
    ))
}

pub fn parse_join_link(link: &str) -> Result<(SignedInvite, Invite, [u8; 32])> {
    let url = url::Url::parse(link.trim()).context("not a valid umbra:// link")?;
    if url.scheme() != "umbra" || url.host_str() != Some("join") {
        bail!("not an umbra://join link");
    }
    let (mut i, mut s) = (None, None);
    for (k, v) in url.query_pairs() {
        match k.as_ref() {
            "i" => i = Some(v.into_owned()),
            "s" => s = Some(v.into_owned()),
            _ => {}
        }
    }
    let si: SignedInvite = uncbor(&unb64(&i.ok_or_else(|| anyhow!("missing invite"))?)?)?;
    let secret: [u8; 32] = unb64(&s.ok_or_else(|| anyhow!("missing secret"))?)?
        .try_into()
        .map_err(|_| anyhow!("bad invite secret"))?;
    let inv = si.verified()?;
    if sha256_url(&secret) != inv.secret_hash {
        bail!("invite secret does not match");
    }
    Ok((si, inv, secret))
}

/// `proof = HMAC-SHA256(invite_secret, "umbra-join-v1" || joiner PeerId bytes)`.
pub fn join_proof(secret: &[u8], joiner: &PeerId) -> Vec<u8> {
    let mut mac = <Hmac<Sha256> as Mac>::new_from_slice(secret).expect("hmac takes any key");
    mac.update(JOIN_PROOF_CONTEXT);
    mac.update(&joiner.to_bytes());
    mac.finalize().into_bytes().to_vec()
}

pub fn check_join_proof(secret: &[u8], joiner: &PeerId, proof: &[u8]) -> bool {
    let mut mac = <Hmac<Sha256> as Mac>::new_from_slice(secret).expect("hmac takes any key");
    mac.update(JOIN_PROOF_CONTEXT);
    mac.update(&joiner.to_bytes());
    mac.verify_slice(proof).is_ok()
}

// ---- effective state ----

/// Why an op was not applied.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OpRejected {
    WrongServer,
    /// Not on top of the current manifest (older ops were folded or dropped
    /// by the owner).
    StaleBase,
    NotAuthorized,
    Invalid(String),
}

/// A manifest plus the roster ops applied on top of it.
#[derive(Debug, Clone)]
pub struct ServerState {
    pub server_id: String,
    pub signed: SignedManifest,
    /// The signed manifest as is.
    pub base: Manifest,
    /// Effective state: `base` with `ops` applied.
    pub manifest: Manifest,
    /// Applied ops (hash, op), in application order.
    pub ops: Vec<(String, RosterOp)>,
    pub deleted: Vec<(String, String)>,
}

impl ServerState {
    pub fn new(server_id: &str, signed: SignedManifest) -> Result<Self> {
        let manifest = signed.manifest()?;
        Ok(Self {
            server_id: server_id.to_string(),
            signed,
            base: manifest.clone(),
            manifest,
            ops: Vec::new(),
            deleted: Vec::new(),
        })
    }

    /// Role of `peer` in the effective roster.
    pub fn role_of(&self, peer: &str) -> Option<Role> {
        self.manifest
            .roster
            .iter()
            .find(|e| e.card.peer_id == peer)
            .map(|e| e.role)
    }

    /// Permission check against the *manifest* (ops are checked against the
    /// manifest at their `base_seq`, which is the current one).
    pub fn base_role(&self, peer: &str) -> Option<Role> {
        role_in(&self.base, peer)
    }

    pub fn can(&self, peer: &str, p: Permission) -> bool {
        role_in(&self.base, peer).is_some_and(|r| role_can(&self.base.policy, r, p))
    }

    pub fn is_banned(&self, peer: &str) -> bool {
        self.manifest.bans.iter().any(|b| b.peer_id == peer)
    }

    pub fn members(&self) -> &[RosterEntry] {
        &self.manifest.roster
    }

    pub fn channels(&self) -> &[Channel] {
        &self.manifest.channels
    }

    pub fn can_read(&self, peer: &str, channel_id: &str) -> bool {
        self.channel_access(peer, channel_id, |c| &c.read)
    }

    pub fn can_post(&self, peer: &str, channel_id: &str) -> bool {
        self.channel_access(peer, channel_id, |c| &c.post)
    }

    fn channel_access(&self, peer: &str, ch: &str, f: impl Fn(&Channel) -> &Vec<Role>) -> bool {
        let (Some(role), Some(c)) = (
            self.role_of(peer),
            self.manifest.channels.iter().find(|c| c.id == ch),
        ) else {
            return false;
        };
        role == Role::Owner || f(c).contains(&role)
    }

    /// Apply one verified op. Rules (servers.md): the signer needs the
    /// permission in the manifest at `base_seq`; `Admit` only for an invite
    /// the signer created; admins act on members only, never on the owner
    /// or other admins; a ban beats an admit of the same PeerId; an unban
    /// needs a newer `base_seq` than the ban.
    pub fn apply_op(&mut self, hash: &str, op: RosterOp, now_ms: i64) -> Result<(), OpRejected> {
        if op.server_id != self.server_id {
            return Err(OpRejected::WrongServer);
        }
        if op.base_seq != self.base.seq {
            return Err(OpRejected::StaleBase);
        }
        if self.ops.iter().any(|(h, _)| h == hash) {
            return Ok(());
        }
        let base = self.base.clone();
        let prior: Vec<RosterOp> = self.ops.iter().map(|(_, o)| o.clone()).collect();
        let signer_role = role_in(&base, &op.signer).ok_or(OpRejected::NotAuthorized)?;
        let need = |p| {
            if role_can(&base.policy, signer_role, p) {
                Ok(())
            } else {
                Err(OpRejected::NotAuthorized)
            }
        };
        let target_ok = |peer: &str| match role_in(&base, peer) {
            Some(Role::Owner) => false,
            Some(Role::Admin) => signer_role == Role::Owner,
            _ => true,
        };
        let inv = |e: &str| OpRejected::Invalid(e.to_string());
        let m = &mut self.manifest;
        match &op.op {
            OpKind::Admit { card, invite } => {
                need(Permission::Invite)?;
                let i = invite.verified().map_err(|e| inv(&e.to_string()))?;
                if i.inviter.peer_id != op.signer || i.server_id != self.server_id {
                    return Err(OpRejected::NotAuthorized);
                }
                if op.ts > i.expires_at || now_ms > i.expires_at + DEFAULT_INVITE_TTL_MS {
                    return Err(inv("invite expired"));
                }
                let card = roster_card(card).map_err(|e| inv(&e.to_string()))?;
                let banned_now = m.bans.iter().any(|b| b.peer_id == card.peer_id);
                let ban_op = prior
                    .iter()
                    .any(|o| matches!(&o.op, OpKind::Ban { peer_id } if *peer_id == card.peer_id));
                if banned_now || ban_op {
                    return Err(inv("banned"));
                }
                if m.roster.iter().any(|e| e.card.peer_id == card.peer_id) {
                    return Err(inv("already a member"));
                }
                if m.roster.len() >= MAX_MEMBERS {
                    return Err(inv("server is full"));
                }
                m.roster.push(RosterEntry {
                    card,
                    role: Role::Member,
                    added_by: op.signer.clone(),
                    added_seq: op.base_seq,
                });
            }
            OpKind::Kick { peer_id } => {
                need(Permission::Kick)?;
                if !target_ok(peer_id) {
                    return Err(OpRejected::NotAuthorized);
                }
                m.roster.retain(|e| e.card.peer_id != *peer_id);
            }
            OpKind::Ban { peer_id } => {
                need(Permission::Ban)?;
                if !target_ok(peer_id) || *peer_id == op.signer {
                    return Err(OpRejected::NotAuthorized);
                }
                m.roster.retain(|e| e.card.peer_id != *peer_id);
                if !m.bans.iter().any(|b| b.peer_id == *peer_id) {
                    m.bans.push(Ban {
                        peer_id: peer_id.clone(),
                        seq: op.base_seq,
                    });
                }
            }
            OpKind::Unban { peer_id } => {
                need(Permission::Ban)?;
                let ban_op = prior
                    .iter()
                    .any(|o| matches!(&o.op, OpKind::Ban { peer_id: p } if p == peer_id));
                let Some(b) = m.bans.iter().find(|b| b.peer_id == *peer_id) else {
                    return Err(inv("not banned"));
                };
                if ban_op || op.base_seq <= b.seq {
                    return Err(inv("an unban needs a newer base_seq than the ban"));
                }
                m.bans.retain(|b| b.peer_id != *peer_id);
            }
            OpKind::DeleteMessage {
                channel_id,
                message_id,
            } => {
                need(Permission::DeleteMessages)?;
                self.deleted.push((channel_id.clone(), message_id.clone()));
            }
            OpKind::CreateChannel { channel } => {
                need(Permission::ManageChannels)?;
                check_name(&channel.name).map_err(|e| inv(&e.to_string()))?;
                if channel.id.len() != 22
                    || m.channels.iter().any(|c| c.id == channel.id)
                    || m.channels.len() >= MAX_CHANNELS
                {
                    return Err(inv("bad channel"));
                }
                m.channels.push(channel.clone());
            }
            OpKind::EditChannel { channel } => {
                need(Permission::ManageChannels)?;
                check_name(&channel.name).map_err(|e| inv(&e.to_string()))?;
                let Some(c) = m.channels.iter_mut().find(|c| c.id == channel.id) else {
                    return Err(inv("unknown channel"));
                };
                *c = channel.clone();
            }
        }
        self.ops.push((hash.to_string(), op));
        Ok(())
    }

    /// The owner folds the effective state into manifest `seq + 1`.
    pub fn next_manifest(&self) -> Manifest {
        let mut m = self.manifest.clone();
        m.server_id = Some(self.server_id.clone());
        m.seq = self.base.seq + 1;
        m.prev = Some(self.signed.hash());
        m.takeover = false;
        m
    }
}

fn role_in(m: &Manifest, peer: &str) -> Option<Role> {
    m.roster
        .iter()
        .find(|e| e.card.peer_id == peer)
        .map(|e| e.role)
}

pub fn role_can(policy: &Policy, role: Role, p: Permission) -> bool {
    match p {
        Permission::Invite => role == Role::Owner || policy.invite.contains(&role),
        _ => matches!(role, Role::Owner | Role::Admin),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    pub(crate) struct Dev {
        pub kp: identity::Keypair,
        pub card: ContactCard,
    }

    pub(crate) fn dev() -> Dev {
        let kp = identity::Keypair::generate_ed25519();
        let ed = kp.public().try_into_ed25519().unwrap();
        let ik = vodozemac::olm::Account::new().curve25519_key();
        let mut card = ContactCard {
            peer_id: kp.public().to_peer_id().to_base58(),
            identity_key: ik.to_base64(),
            ed25519_key: b64(&ed.to_bytes()),
            addrs: vec![],
            display_name: None,
            sig: None,
        };
        qr::sign_card(&kp, &mut card).unwrap();
        Dev { kp, card }
    }

    fn genesis(owner: &ServerKey, rec: &ServerKey, dev: &Dev) -> SignedManifest {
        let m = Manifest {
            v: 1,
            server_id: None,
            seq: 0,
            prev: None,
            owner: owner.public_b64(),
            recovery: vec![rec.public_b64()],
            name: "S".into(),
            icon: None,
            channels: vec![Channel::new_text("general")],
            roster: vec![RosterEntry {
                card: dev.card.clone(),
                role: Role::Owner,
                added_by: dev.card.peer_id.clone(),
                added_seq: 0,
            }],
            bans: vec![],
            hosts: vec![],
            packs: vec![],
            policy: Policy::default(),
            takeover: false,
        };
        SignedManifest::sign(&m, owner).unwrap()
    }

    fn op(st: &ServerState, by: &Dev, kind: OpKind) -> (String, RosterOp) {
        let o = RosterOp {
            server_id: st.server_id.clone(),
            base_seq: st.base.seq,
            op: kind,
            ts: 1,
            signer: by.card.peer_id.clone(),
            signer_ed: by.card.ed25519_key.clone(),
        };
        let s = SignedOp::sign(&o, &by.kp).unwrap();
        (s.hash(), s.verified().unwrap())
    }

    #[test]
    fn manifests_chain_and_refuse_rollbacks_and_forks() {
        let (k, r, o) = (ServerKey::generate(), ServerKey::generate(), dev());
        let g = genesis(&k, &r, &o);
        let sid = check_genesis(&g).unwrap();
        assert_eq!(sid.len(), 43);
        let st = ServerState::new(&sid, g.clone()).unwrap();
        let mut m1 = st.next_manifest();
        m1.name = "S1".into();
        let s1 = SignedManifest::sign(&m1, &k).unwrap();
        assert_eq!(check_next(&sid, &g, &s1).unwrap(), ManifestCheck::Accept);
        assert_eq!(
            check_next(&sid, &s1, &s1).unwrap(),
            ManifestCheck::Duplicate
        );
        assert_eq!(
            check_next(&sid, &s1, &g).unwrap(),
            ManifestCheck::Rollback { have: 1, got: 0 }
        );
        let mut m1b = m1.clone();
        m1b.name = "other".into();
        let s1b = SignedManifest::sign(&m1b, &k).unwrap();
        assert_eq!(
            check_next(&sid, &s1, &s1b).unwrap(),
            ManifestCheck::Fork { seq: 1 }
        );
        // Not signed by the owner key: refused.
        let stranger = ServerKey::generate();
        assert!(check_next(&sid, &g, &SignedManifest::sign(&m1, &stranger).unwrap()).is_err());
        // Wrong prev: refused.
        let mut bad = m1.clone();
        bad.prev = Some("x".into());
        assert!(check_next(&sid, &g, &SignedManifest::sign(&bad, &k).unwrap()).is_err());
        // A takeover by a recovery key, naming a new owner.
        let k2 = ServerKey::generate();
        let mut t = ServerState::new(&sid, s1.clone()).unwrap().next_manifest();
        t.owner = k2.public_b64();
        t.takeover = true;
        let st2 = SignedManifest::sign(&t, &r).unwrap();
        assert_eq!(check_next(&sid, &s1, &st2).unwrap(), ManifestCheck::Accept);
        t.takeover = false;
        assert!(check_next(&sid, &s1, &SignedManifest::sign(&t, &r).unwrap()).is_err());
        // A gap is fine when the same owner key signs it.
        let mut m3 = m1.clone();
        m3.seq = 3;
        m3.prev = Some("unknown".into());
        let s3 = SignedManifest::sign(&m3, &k).unwrap();
        assert_eq!(check_next(&sid, &g, &s3).unwrap(), ManifestCheck::Accept);
        // A joiner anchors on the owner key from the invite.
        check_first(&sid, &k.public_b64(), &s1).unwrap();
        check_first(&sid, &k.public_b64(), &g).unwrap();
        assert!(check_first(&sid, &k2.public_b64(), &s1).is_err());
        assert!(check_first("other", &k.public_b64(), &s1).is_err());
    }

    #[test]
    fn manifests_need_recovery_keys_and_signed_cards() {
        let (k, r, o) = (ServerKey::generate(), ServerKey::generate(), dev());
        let mut m = genesis(&k, &r, &o).manifest().unwrap();
        m.recovery.clear();
        assert!(validate_manifest(&m).is_err());
        m.recovery = vec![r.public_b64(); 4];
        assert!(validate_manifest(&m).is_err());
        m.recovery = vec![k.public_b64()];
        assert!(
            validate_manifest(&m).is_err(),
            "recovery key can't be the owner key"
        );
        m.recovery = vec![r.public_b64()];
        m.roster[0].card.sig = None;
        assert!(validate_manifest(&m).is_err());
        assert!(roster_card(&dev().card).is_ok());
        let mut unsigned = dev().card;
        unsigned.sig = None;
        assert!(roster_card(&unsigned).is_err());
    }

    fn invite(st: &ServerState, k: &ServerKey, by: &Dev, expires_at: i64) -> SignedInvite {
        let inv = Invite {
            server_id: st.server_id.clone(),
            owner_key: k.public_b64(),
            server_name: "S".into(),
            inviter: by.card.clone(),
            invite_id: "i1".into(),
            secret_hash: sha256_url(&[7u8; 32]),
            expires_at,
            max_uses: 1,
            needs_approval: false,
            hosts: vec![],
        };
        SignedInvite::sign(&inv, &by.kp).unwrap()
    }

    #[test]
    fn ops_follow_permissions_and_ban_rules() {
        let (k, r, o) = (ServerKey::generate(), ServerKey::generate(), dev());
        let g = genesis(&k, &r, &o);
        let sid = check_genesis(&g).unwrap();
        let (admin, member, joiner) = (dev(), dev(), dev());
        // Manifest 1: an admin and a member.
        let mut m = ServerState::new(&sid, g.clone()).unwrap().next_manifest();
        for (d, role) in [(&admin, Role::Admin), (&member, Role::Member)] {
            m.roster.push(RosterEntry {
                card: d.card.clone(),
                role,
                added_by: o.card.peer_id.clone(),
                added_seq: 1,
            });
        }
        let s1 = SignedManifest::sign(&m, &k).unwrap();
        let mut st = ServerState::new(&sid, s1).unwrap();
        let now = 0;

        // Members can't invite by default; admins can.
        assert!(!st.can(&member.card.peer_id, Permission::Invite));
        assert!(st.can(&admin.card.peer_id, Permission::Invite));
        let inv = invite(&st, &k, &admin, 1_000);
        let (h, o1) = op(
            &st,
            &member,
            OpKind::Admit {
                card: joiner.card.clone(),
                invite: inv.clone(),
            },
        );
        assert_eq!(st.apply_op(&h, o1, now), Err(OpRejected::NotAuthorized));
        // An admin can't admit with someone else's invite.
        let other_inv = invite(&st, &k, &o, 1_000);
        let (h, o2) = op(
            &st,
            &admin,
            OpKind::Admit {
                card: joiner.card.clone(),
                invite: other_inv,
            },
        );
        assert_eq!(st.apply_op(&h, o2, now), Err(OpRejected::NotAuthorized));
        let (h, o3) = op(
            &st,
            &admin,
            OpKind::Admit {
                card: joiner.card.clone(),
                invite: inv.clone(),
            },
        );
        st.apply_op(&h, o3, now).unwrap();
        assert_eq!(st.role_of(&joiner.card.peer_id), Some(Role::Member));

        // Admins can't kick the owner or another admin; members can't kick.
        let (h, k1) = op(
            &st,
            &admin,
            OpKind::Kick {
                peer_id: o.card.peer_id.clone(),
            },
        );
        assert_eq!(st.apply_op(&h, k1, now), Err(OpRejected::NotAuthorized));
        let (h, k2) = op(
            &st,
            &member,
            OpKind::Kick {
                peer_id: joiner.card.peer_id.clone(),
            },
        );
        assert_eq!(st.apply_op(&h, k2, now), Err(OpRejected::NotAuthorized));

        // A ban beats an admit of the same peer, in either order.
        let late = dev();
        let (h, b) = op(
            &st,
            &admin,
            OpKind::Ban {
                peer_id: late.card.peer_id.clone(),
            },
        );
        st.apply_op(&h, b, now).unwrap();
        let (h, a) = op(
            &st,
            &admin,
            OpKind::Admit {
                card: late.card.clone(),
                invite: inv.clone(),
            },
        );
        assert!(matches!(
            st.apply_op(&h, a, now),
            Err(OpRejected::Invalid(_))
        ));
        assert!(st.is_banned(&late.card.peer_id));
        // An unban on the same base as the ban is refused.
        let (h, u) = op(
            &st,
            &admin,
            OpKind::Unban {
                peer_id: late.card.peer_id.clone(),
            },
        );
        assert!(matches!(
            st.apply_op(&h, u, now),
            Err(OpRejected::Invalid(_))
        ));

        // Channels: admins only.
        let (h, c) = op(
            &st,
            &member,
            OpKind::CreateChannel {
                channel: Channel::new_text("x"),
            },
        );
        assert_eq!(st.apply_op(&h, c, now), Err(OpRejected::NotAuthorized));
        let ch = Channel::new_text("random");
        let (h, c) = op(
            &st,
            &admin,
            OpKind::CreateChannel {
                channel: ch.clone(),
            },
        );
        st.apply_op(&h, c, now).unwrap();
        assert!(st.can_post(&member.card.peer_id, &ch.id));

        // The owner folds it all into manifest 2; the unban then works.
        let s2 = SignedManifest::sign(&st.next_manifest(), &k).unwrap();
        assert_eq!(
            check_next(&sid, &st.signed, &s2).unwrap(),
            ManifestCheck::Accept
        );
        let mut st2 = ServerState::new(&sid, s2).unwrap();
        assert_eq!(st2.members().len(), 4);
        // Old ops don't apply on the new base.
        let (h, stale) = op(
            &st,
            &admin,
            OpKind::Kick {
                peer_id: joiner.card.peer_id.clone(),
            },
        );
        assert_eq!(st2.apply_op(&h, stale, now), Err(OpRejected::StaleBase));
        let (h, u) = op(
            &st2,
            &admin,
            OpKind::Unban {
                peer_id: late.card.peer_id.clone(),
            },
        );
        st2.apply_op(&h, u, now).unwrap();
        assert!(!st2.is_banned(&late.card.peer_id));
    }

    #[test]
    fn invites_round_trip_and_bind_the_joiner() {
        let (k, r, o) = (ServerKey::generate(), ServerKey::generate(), dev());
        let g = genesis(&k, &r, &o);
        let sid = check_genesis(&g).unwrap();
        let st = ServerState::new(&sid, g).unwrap();
        let si = invite(&st, &k, &o, 1_000);
        let link = build_join_link(&si, &[7u8; 32]).unwrap();
        let (_, inv, secret) = parse_join_link(&link).unwrap();
        assert_eq!(inv.server_id, sid);
        assert!(parse_join_link(&build_join_link(&si, &[8u8; 32]).unwrap()).is_err());
        let mut tampered = si.clone();
        tampered.bytes[5] ^= 1;
        assert!(tampered.verified().is_err());
        let j = dev().kp.public().to_peer_id();
        let proof = join_proof(&secret, &j);
        assert!(check_join_proof(&secret, &j, &proof));
        assert!(!check_join_proof(
            &secret,
            &dev().kp.public().to_peer_id(),
            &proof
        ));
        assert_eq!(key_fingerprint(&k.public_b64()).unwrap().len(), 39);
    }
}
