//! Wire protocols carried over libp2p request/response (CBOR codec).
//!
//! * `/umbra/msg/1`: `MsgRequest` -> `MsgResponse`. The response *is* the
//!   delivery ack; it travels over the Noise-authenticated connection.
//! * `/umbra/keys/1`: `KeysRequest` -> `KeysResponse` (Olm one-time key claim).
//!
//! The Olm plaintext is a JSON [`Payload`].

use serde::{Deserialize, Serialize};

use crate::api::types::{ContactCard, SignalKind};
use crate::engine::servers::ServerPayloadKind;

pub const MSG_PROTOCOL: &str = "/umbra/msg/1";
pub const KEYS_PROTOCOL: &str = "/umbra/keys/1";
pub const KAD_PROTOCOL: &str = "/umbra/kad/1.0.0";
pub const IDENTIFY_PROTOCOL: &str = "/umbra/id/1.0.0";

pub const MAX_TEXT_BYTES: usize = 64 * 1024;
pub const MAX_SIGNAL_BYTES: usize = 64 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MsgRequest {
    /// Introduce ourselves (sent once per connection). Unknown peers turn
    /// into a `ContactRequest` event on the receiving side.
    Hello { card: ContactCard },
    /// Olm message: `t` = 0 pre-key, 1 normal.
    Olm {
        t: u8,
        #[serde(with = "serde_bytes")]
        c: Vec<u8>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RejectReason {
    /// Recipient has not added us as a contact (yet).
    NotAContact,
    /// Recipient could not decrypt (lost/stale session): start a new one.
    DecryptFailed,
    Malformed,
    Internal,
    /// The sender's identity key differs from the one we trust for it.
    KeyMismatch,
    /// Too many requests; retry later.
    RateLimited,
    /// Server-scoped payload from a peer that is not a co-member of that
    /// server, or whose key for it is unresolved or unsigned.
    NotAMember,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum MsgResponse {
    /// Hello accepted (sender is a contact).
    Ok,
    /// Olm message decrypted and stored / dispatched.
    Delivered,
    Rejected(RejectReason),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum KeysRequest {
    ClaimOneTimeKey,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum KeysResponse {
    Key {
        identity_key: String,
        one_time_key: String,
    },
    Refused(RejectReason),
}

/// Olm plaintext.
///
/// `seq`, `prev` and `expires_at` are optional, reserved envelope fields
/// (gap / deletion detection and disappearing messages; semantics TBD by
/// Jal's designs). They are omitted when unset, and older cores ignore them
/// (serde skips unknown fields), so adding them is wire-compatible both ways.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Payload {
    pub v: u8,
    /// Intended recipient PeerId (guards against misrouted ciphertext).
    pub to: String,
    pub id: String,
    pub ts: i64,
    pub body: PayloadBody,
    /// Per-sender, monotonically increasing sequence number.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seq: Option<u64>,
    /// Hash of the sender's previous message in the same stream (hash chain),
    /// unpadded base64.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prev: Option<String>,
    /// Sender-requested expiry, ms since the Unix epoch.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<i64>,
}

/// The optional envelope fields of a [`Payload`], as stored in the outbox
/// and `messages` and passed through the engine API.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnvelopeMeta {
    pub seq: Option<u64>,
    pub prev: Option<String>,
    pub expires_at: Option<i64>,
}

/// Max length of `Payload.prev` (a hash, base64).
pub const MAX_PREV_HASH_CHARS: usize = 128;

impl Payload {
    pub fn new(to: String, id: String, ts: i64, body: PayloadBody, meta: EnvelopeMeta) -> Self {
        Self {
            v: 1,
            to,
            id,
            ts,
            body,
            seq: meta.seq,
            prev: meta.prev,
            expires_at: meta.expires_at,
        }
    }

    pub fn meta(&self) -> EnvelopeMeta {
        EnvelopeMeta {
            seq: self.seq,
            prev: self.prev.clone(),
            expires_at: self.expires_at,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PayloadBody {
    Text {
        text: String,
    },
    Signal {
        call_id: String,
        kind: SignalKind,
        payload: String,
    },
    /// Server-scoped payload (Phase 0 carrier, see docs/core-api.md).
    /// Accepted only from co-members of `server_id`. `data` is unpadded
    /// base64 and opaque to Phase 0.
    Server {
        server_id: String,
        kind: ServerPayloadKind,
        data: String,
    },
}
