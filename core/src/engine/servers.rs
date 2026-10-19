//! Community servers, Phase 0 (docs/servers.md): local membership records,
//! the co-member trust class and the server-scoped Olm payload carrier.
//!
//! These are engine-level (Rust) types. They are deliberately not part of
//! the FFI API yet; Phase 1 server logic in core drives them from rosters.

use serde::{Deserialize, Serialize};

/// Which of OUR identities a server membership uses. An explicit choice on
/// every membership; nothing defaults silently.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ServerIdentity {
    /// The user's normal identity (same PeerId / keys as for contacts).
    Default,
    /// A separate identity used only in this server. Reserved: Phase 0
    /// refuses it (it needs its own device key, Olm account and network
    /// identity). Rules once implemented: it is never linked to the default
    /// identity in cards, logs or outbox rows.
    Dedicated,
}

impl ServerIdentity {
    pub fn as_str(self) -> &'static str {
        match self {
            ServerIdentity::Default => "default",
            ServerIdentity::Dedicated => "dedicated",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "default" => Some(ServerIdentity::Default),
            "dedicated" => Some(ServerIdentity::Dedicated),
            _ => None,
        }
    }
}

/// History visibility for new members (servers.md "History for new
/// members"). A per-server setting; Phase 0 only stores it and nothing in
/// core reads it yet. `None` on a membership = not set (server policy decides).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HistoryVisibility {
    /// New members read from the moment they joined.
    Joined,
    /// Admins forward recent history keys to new members.
    Shared,
}

impl HistoryVisibility {
    pub fn as_str(self) -> &'static str {
        match self {
            HistoryVisibility::Joined => "joined",
            HistoryVisibility::Shared => "shared",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "joined" => Some(HistoryVisibility::Joined),
            "shared" => Some(HistoryVisibility::Shared),
            _ => None,
        }
    }
}

/// Local membership record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServerMembership {
    pub server_id: String,
    pub identity: ServerIdentity,
    pub history_visibility: Option<HistoryVisibility>,
    pub joined_at_ms: i64,
}

/// A peer we share `server_id` with. Distinct from `Contact`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoMember {
    pub server_id: String,
    pub peer_id: String,
    pub display_name: String,
    pub identity_key: String,
    pub ed25519_key: String,
    pub addrs: Vec<String>,
    pub added_at_ms: i64,
    /// We hold a valid card v2 signature for `identity_key`.
    pub signed_card: bool,
    /// An untrusted different identity key is pending (see `KeyChanged`).
    pub key_changed: bool,
    pub pending_identity_key: Option<String>,
    /// This peer is also a contact (informational; never implied).
    pub is_contact: bool,
}

/// Kinds of server-scoped Olm payloads (servers.md "Wire protocol
/// additions"). Phase 0 carries them opaquely; Phase 1 defines `data`.
/// New kinds go at the end.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ServerPayloadKind {
    ServerKey,
    ServerKeyForward,
    ServerKeyRequest,
    ServerManifest,
    ServerOp,
}

/// Max bytes of `data` in one server payload.
pub const MAX_SERVER_PAYLOAD_BYTES: usize = 64 * 1024;

/// A server payload received from a co-member, after all checks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServerInbound {
    /// Olm sender (authenticated by Noise + the Olm session), never from the plaintext.
    pub peer_id: String,
    pub server_id: String,
    pub kind: ServerPayloadKind,
    pub data: Vec<u8>,
    pub id: String,
    pub ts: i64,
    /// Optional envelope fields (seq / prev hash / expiry), as sent.
    pub meta: crate::engine::proto::EnvelopeMeta,
}

/// Server ids are unpadded base64url (SHA-256 => 43 chars); accept any
/// reasonable base64url string so tests and later formats work.
pub fn validate_server_id(s: &str) -> anyhow::Result<()> {
    if s.is_empty()
        || s.len() > 128
        || !s
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
    {
        anyhow::bail!("invalid server id");
    }
    Ok(())
}
