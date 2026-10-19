//! `/umbra/srv/1`: members and hosts (docs/servers.md "Hosts", "Operator
//! duties", "Wire protocol additions"). CBOR over request-response, like
//! `/umbra/msg/1`. The host side is the `umbra-host` program; member-side
//! use (pushing envelopes, audits) comes with Megolm and host audits.
//!
//! Everything a host signs uses its libp2p Ed25519 device key with a
//! per-type context, so a member can check a receipt or refusal against the
//! host's PeerId alone.

use anyhow::{bail, Result};
use libp2p::identity::{self, PublicKey};
use libp2p::PeerId;
use serde::{Deserialize, Serialize};

use crate::engine::server_model::{cbor, sha256_url, signed_message};

pub const SRV_PROTOCOL: &str = "/umbra/srv/1";

const HOST_INFO_CONTEXT: &[u8] = b"umbra-host-info-v1";
const RECEIPT_CONTEXT: &[u8] = b"umbra-host-receipt-v1";
const REFUSAL_CONTEXT: &[u8] = b"umbra-host-refusal-v1";
const GRANT_CONTEXT: &[u8] = b"umbra-host-grant-v1";
const NOTICE_CONTEXT: &[u8] = b"umbra-operator-notice-v1";

/// Megolm ciphertext of a padded 64 KiB plaintext, plus headroom.
pub const MAX_ENVELOPE_BYTES: usize = 96 * 1024;
/// Max Olm ciphertext relayed for an offline member.
pub const MAX_RELAY_BYTES: usize = 512 * 1024;
pub const MAX_FETCH: u32 = 500;
pub const MAX_GRANT_MEMBERS: usize = crate::engine::server_model::MAX_MEMBERS;

/// A channel message as a host stores it. No sender field: members find the
/// sender from the Megolm session binding.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Envelope {
    pub server_id: String,
    pub channel_id: String,
    pub session_id: String,
    /// Coarse expiry, unix ms; the host drops (and stops serving) it then.
    pub exp_bucket: Option<i64>,
    #[serde(with = "serde_bytes")]
    pub megolm: Vec<u8>,
}

impl Envelope {
    /// `EnvHash`: SHA-256 of the ciphertext, base64url.
    pub fn hash(&self) -> String {
        sha256_url(&self.megolm)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoredEnvelope {
    pub cursor: u64,
    pub env: Envelope,
}

/// The host ACL for one server, signed by the owner or an admin device.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HostGrant {
    pub server_id: String,
    /// Manifest seq the grant was made at; a newer grant replaces an older.
    pub seq: u64,
    pub members: Vec<String>,
    /// Bytes.
    pub quota: u64,
    /// Days, at most 30.
    pub retention: u32,
    pub ts: i64,
    /// Signer PeerId and its Ed25519 key (base64); the device key signs.
    pub signer: String,
    pub signer_ed: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SignedGrant {
    #[serde(with = "serde_bytes")]
    pub bytes: Vec<u8>,
    #[serde(with = "serde_bytes")]
    pub sig: Vec<u8>,
}

impl SignedGrant {
    pub fn sign(g: &HostGrant, device: &identity::Keypair) -> Result<Self> {
        let bytes = cbor(g)?;
        let sig = device.sign(&signed_message(GRANT_CONTEXT, &bytes))?;
        Ok(Self { bytes, sig })
    }

    /// Decode and check the signature and that the signer key derives the
    /// signer PeerId.
    pub fn verified(&self) -> Result<HostGrant> {
        if self.bytes.len() > 64 * 1024 {
            bail!("grant too large");
        }
        let g: HostGrant = crate::engine::server_model::uncbor(&self.bytes)?;
        let pk = crate::engine::server_model::ed_public(&g.signer_ed)?;
        let pk = PublicKey::from(pk);
        if pk.to_peer_id().to_base58() != g.signer {
            bail!("grant signer key does not match its PeerId");
        }
        if !pk.verify(&signed_message(GRANT_CONTEXT, &self.bytes), &self.sig) {
            bail!("bad grant signature");
        }
        if g.members.len() > MAX_GRANT_MEMBERS {
            bail!("too many members in grant");
        }
        Ok(g)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Scope {
    Blob,
    Envelope,
    Peer,
    Server,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RefusalReason {
    Denylist,
    Takedown { notice_id: String },
}

/// Signed refusal receipt. `statement` is the DSA Art. 17 statement of
/// reasons.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Refused {
    pub id: String,
    pub scope: Scope,
    pub reason: RefusalReason,
    pub statement: Option<String>,
    pub ts: i64,
    #[serde(with = "serde_bytes")]
    pub sig: Vec<u8>,
}

fn sign_with(key: &identity::Keypair, ctx: &[u8], body: &impl Serialize) -> Result<Vec<u8>> {
    Ok(key.sign(&signed_message(ctx, &cbor(body)?))?)
}

fn verify_with(
    host: &PeerId,
    ctx: &[u8],
    body: &impl Serialize,
    sig: &[u8],
    key: &PublicKey,
) -> Result<bool> {
    Ok(key.to_peer_id() == *host && key.verify(&signed_message(ctx, &cbor(body)?), sig))
}

impl Refused {
    fn body(&self) -> (&str, Scope, &RefusalReason, &Option<String>, i64) {
        (&self.id, self.scope, &self.reason, &self.statement, self.ts)
    }
    pub fn sign(
        key: &identity::Keypair,
        id: &str,
        scope: Scope,
        reason: RefusalReason,
        statement: Option<String>,
        ts: i64,
    ) -> Result<Self> {
        let mut r = Self {
            id: id.to_string(),
            scope,
            reason,
            statement,
            ts,
            sig: Vec::new(),
        };
        r.sig = sign_with(key, REFUSAL_CONTEXT, &r.body())?;
        Ok(r)
    }
    pub fn verify(&self, host: &PeerId, key: &PublicKey) -> Result<bool> {
        verify_with(host, REFUSAL_CONTEXT, &self.body(), &self.sig, key)
    }
}

/// Signed `Stored` / `Queued` receipt: the host took the item.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Receipt {
    /// `server_id` for envelopes; recipient PeerId for relays.
    pub scope_id: String,
    /// `EnvHash`, or SHA-256 of the relayed ciphertext.
    pub hash: String,
    pub cursor: u64,
    pub ts: i64,
    #[serde(with = "serde_bytes")]
    pub sig: Vec<u8>,
}

impl Receipt {
    fn body(&self) -> (&str, &str, u64, i64) {
        (&self.scope_id, &self.hash, self.cursor, self.ts)
    }
    pub fn sign(
        key: &identity::Keypair,
        scope_id: &str,
        hash: &str,
        cursor: u64,
        ts: i64,
    ) -> Result<Self> {
        let mut r = Self {
            scope_id: scope_id.to_string(),
            hash: hash.to_string(),
            cursor,
            ts,
            sig: Vec::new(),
        };
        r.sig = sign_with(key, RECEIPT_CONTEXT, &r.body())?;
        Ok(r)
    }
    pub fn verify(&self, host: &PeerId, key: &PublicKey) -> Result<bool> {
        verify_with(host, RECEIPT_CONTEXT, &self.body(), &self.sig, key)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LoggingInfo {
    pub ip_logs: bool,
    pub retention_hours: u32,
}

/// Signed operator information, shown in the join preview and the host
/// info sheet.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HostInfo {
    pub peer_id: String,
    pub operator_name: Option<String>,
    pub contact: String,
    pub dmca_agent: Option<String>,
    pub aup_sha256: String,
    pub aup_version: u32,
    pub logging: LoggingInfo,
    pub ts: i64,
    /// Protobuf-encoded libp2p public key of the host (so the info verifies
    /// on its own; it must derive `peer_id`).
    #[serde(with = "serde_bytes")]
    pub public_key: Vec<u8>,
    #[serde(with = "serde_bytes")]
    pub sig: Vec<u8>,
}

impl HostInfo {
    #[allow(clippy::type_complexity)]
    fn body(
        &self,
    ) -> (
        &str,
        &Option<String>,
        &str,
        &Option<String>,
        &str,
        u32,
        &LoggingInfo,
        i64,
        &[u8],
    ) {
        (
            &self.peer_id,
            &self.operator_name,
            &self.contact,
            &self.dmca_agent,
            &self.aup_sha256,
            self.aup_version,
            &self.logging,
            self.ts,
            &self.public_key,
        )
    }
    pub fn sign(mut self, key: &identity::Keypair) -> Result<Self> {
        self.peer_id = key.public().to_peer_id().to_base58();
        self.public_key = key.public().encode_protobuf();
        self.sig = sign_with(key, HOST_INFO_CONTEXT, &self.body())?;
        Ok(self)
    }
    pub fn verify(&self) -> Result<bool> {
        let key = PublicKey::try_decode_protobuf(&self.public_key)?;
        let host: PeerId = self.peer_id.parse()?;
        verify_with(&host, HOST_INFO_CONTEXT, &self.body(), &self.sig, &key)
    }
}

/// Statement of reasons for a takedown, queued to the server owner (the
/// grant signer) and to the PeerId that pushed the item.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OperatorNotice {
    pub notice_id: String,
    pub server_id: Option<String>,
    pub items: Vec<(Scope, String)>,
    pub statement: Option<String>,
    pub ts: i64,
    #[serde(with = "serde_bytes")]
    pub sig: Vec<u8>,
}

impl OperatorNotice {
    #[allow(clippy::type_complexity)]
    fn body(
        &self,
    ) -> (
        &str,
        &Option<String>,
        &Vec<(Scope, String)>,
        &Option<String>,
        i64,
    ) {
        (
            &self.notice_id,
            &self.server_id,
            &self.items,
            &self.statement,
            self.ts,
        )
    }
    pub fn sign(mut self, key: &identity::Keypair) -> Result<Self> {
        self.sig = sign_with(key, NOTICE_CONTEXT, &self.body())?;
        Ok(self)
    }
    pub fn verify(&self, host: &PeerId, key: &PublicKey) -> Result<bool> {
        verify_with(host, NOTICE_CONTEXT, &self.body(), &self.sig, key)
    }
}

/// An Olm payload queued for an offline member.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RelayedItem {
    pub from: String,
    pub t: u8,
    #[serde(with = "serde_bytes")]
    pub c: Vec<u8>,
    pub ts: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SrvRequest {
    Push {
        env: Envelope,
    },
    Fetch {
        server_id: String,
        channel_id: String,
        /// Cursor to continue after (0 = from the start).
        after: u64,
        limit: u32,
    },
    GetEnv {
        server_id: String,
        hashes: Vec<String>,
    },
    Info,
    Relay {
        to: String,
        t: u8,
        #[serde(with = "serde_bytes")]
        c: Vec<u8>,
        exp_bucket: Option<i64>,
    },
    FetchRelayed,
    Grant {
        grant: SignedGrant,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SrvReject {
    /// Not in the newest grant for that server (or denylisted silently).
    NotAMember,
    Quota,
    TooLarge,
    Malformed,
    RateLimited,
    /// A grant older than the one we have, or signed by another key.
    StaleGrant,
    Internal,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SrvResponse {
    Ok,
    Stored {
        cursor: u64,
        receipt: Receipt,
    },
    Refused {
        receipt: Refused,
    },
    Rejected(SrvReject),
    Envelopes {
        items: Vec<StoredEnvelope>,
        refused: Vec<Refused>,
        next: Option<u64>,
    },
    Info(HostInfo),
    Queued {
        receipt: Receipt,
    },
    Relayed {
        items: Vec<RelayedItem>,
        notices: Vec<OperatorNotice>,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn signatures_bind_the_host_and_the_fields() {
        let k = identity::Keypair::generate_ed25519();
        let host = k.public().to_peer_id();
        let r = Refused::sign(&k, "h", Scope::Envelope, RefusalReason::Denylist, None, 5).unwrap();
        assert!(r.verify(&host, &k.public()).unwrap());
        let mut bad = r.clone();
        bad.scope = Scope::Blob;
        assert!(!bad.verify(&host, &k.public()).unwrap());
        let other = identity::Keypair::generate_ed25519();
        assert!(!r.verify(&other.public().to_peer_id(), &k.public()).unwrap());

        let info = HostInfo {
            peer_id: String::new(),
            operator_name: None,
            contact: "abuse@example.org".into(),
            dmca_agent: None,
            aup_sha256: "x".into(),
            aup_version: 1,
            logging: LoggingInfo {
                ip_logs: false,
                retention_hours: 24,
            },
            ts: 1,
            public_key: vec![],
            sig: vec![],
        }
        .sign(&k)
        .unwrap();
        assert!(info.verify().unwrap());
        let mut lie = info.clone();
        lie.logging.ip_logs = true;
        assert!(!lie.verify().unwrap());

        let g = HostGrant {
            server_id: "s".into(),
            seq: 1,
            members: vec![host.to_base58()],
            quota: 10,
            retention: 30,
            ts: 1,
            signer: host.to_base58(),
            signer_ed: crate::engine::util::b64(&k.public().try_into_ed25519().unwrap().to_bytes()),
        };
        let sg = SignedGrant::sign(&g, &k).unwrap();
        assert_eq!(sg.verified().unwrap(), g);
        let mut forged = g.clone();
        forged.signer = other.public().to_peer_id().to_base58();
        assert!(SignedGrant::sign(&forged, &k).unwrap().verified().is_err());
    }
}
