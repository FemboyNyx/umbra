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
    /// Ask to join a server with an invite (servers.md "Invites and
    /// joining"). `proof = HMAC(invite_secret, "umbra-join-v1" || PeerId)`,
    /// so a stolen proof is useless to anyone else. Sent before any session
    /// exists; answered `Ok` (admitted or waiting for approval) or
    /// `Rejected(InviteInvalid | NotAuthorized | Banned | RateLimited)`.
    JoinHello {
        card: ContactCard,
        server_id: String,
        invite_id: String,
        #[serde(with = "serde_bytes")]
        proof: Vec<u8>,
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
    /// The payload's `seq` is at or below one already accepted from the
    /// sender (a replay, or the sender's state was rolled back). The sender
    /// should jump its sequence numbers and start a fresh session.
    Replayed,
    /// `JoinHello` with an unknown, expired, revoked or used-up invite, or a
    /// bad proof.
    InviteInvalid,
    /// The recipient may not admit anyone (no invite permission, server
    /// frozen or full, or we were removed).
    NotAuthorized,
    /// The joiner is banned from the server.
    Banned,
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

// ---- padding (docs/message-protection.md#padding) ----

/// Olm plaintext size buckets: 256 B, 1 KiB, 4 KiB, 16 KiB, 64 KiB.
pub const PAD_BUCKETS: [usize; 5] = [256, 1024, 4096, 16 * 1024, 64 * 1024];

/// Bucket a plaintext of `len` bytes is padded to. Past the largest bucket
/// (possible: a 64 KiB text or signal plus its JSON framing and escaping)
/// it rounds up to a multiple of 64 KiB, still far under the 1 MiB codec cap.
pub fn padded_len(len: usize) -> usize {
    let top = PAD_BUCKETS[PAD_BUCKETS.len() - 1];
    PAD_BUCKETS
        .iter()
        .copied()
        .find(|b| *b >= len)
        .unwrap_or_else(|| len.div_ceil(top) * top)
}

/// Serialize a [`Payload`] for Olm, padded to its bucket with trailing JSON
/// whitespace. Every JSON parser (including older cores, which use
/// `serde_json::from_slice`) skips trailing whitespace, so this is
/// wire-compatible in both directions.
pub fn encode_payload(p: &Payload) -> serde_json::Result<Vec<u8>> {
    let mut v = serde_json::to_vec(p)?;
    let n = padded_len(v.len());
    v.resize(n, b' ');
    Ok(v)
}

/// Inverse of [`encode_payload`]; also accepts unpadded payloads from
/// older senders.
pub fn decode_payload(pt: &[u8]) -> serde_json::Result<Payload> {
    let end = pt
        .iter()
        .rposition(|b| !matches!(b, b' ' | b'\n' | b'\r' | b'\t'))
        .map_or(0, |i| i + 1);
    serde_json::from_slice(&pt[..end])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text(n: usize) -> Payload {
        Payload::new(
            "to".into(),
            "id".into(),
            1,
            PayloadBody::Text {
                text: "x".repeat(n),
            },
            EnvelopeMeta::default(),
        )
    }

    #[test]
    fn padding_hits_buckets_and_round_trips() {
        assert_eq!(padded_len(0), 256);
        assert_eq!(padded_len(256), 256);
        assert_eq!(padded_len(257), 1024);
        assert_eq!(padded_len(64 * 1024), 64 * 1024);
        assert_eq!(padded_len(64 * 1024 + 1), 128 * 1024);
        for n in [0, 1, 200, 300, 5000, 20_000, MAX_TEXT_BYTES] {
            let p = text(n);
            let raw = serde_json::to_vec(&p).unwrap();
            let enc = encode_payload(&p).unwrap();
            assert_eq!(enc.len(), padded_len(raw.len()));
            assert!(enc.len() >= raw.len());
            // New and old (plain serde_json) decoders both accept it.
            assert_eq!(decode_payload(&enc).unwrap().id, "id");
            assert!(serde_json::from_slice::<Payload>(&enc).is_ok());
            // And unpadded input from old senders.
            assert!(decode_payload(&raw).is_ok());
        }
        // Lengths inside a bucket are indistinguishable.
        assert_eq!(
            encode_payload(&text(10)).unwrap().len(),
            encode_payload(&text(150)).unwrap().len()
        );
    }

    #[test]
    fn largest_signal_stays_bounded() {
        let p = Payload::new(
            "to".into(),
            "id".into(),
            1,
            PayloadBody::Signal {
                call_id: "c".repeat(128),
                kind: SignalKind::Offer,
                payload: "y".repeat(MAX_SIGNAL_BYTES),
            },
            EnvelopeMeta::default(),
        );
        let enc = encode_payload(&p).unwrap();
        assert_eq!(enc.len(), 128 * 1024);
        assert_eq!(decode_payload(&enc).unwrap().id, "id");
    }
}
