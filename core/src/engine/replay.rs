//! Anti-replay helpers (docs/message-protection.md#anti-replay).
//!
//! * Megolm: vodozemac re-decrypts any index at or after the first known
//!   one, so core keeps every accepted `(session_id, message_index)` and
//!   drops repeats ([`crate::engine::core::Core::accept_group_message`]).
//! * Sequence numbers: a `Payload.seq` at or below the highest seen from that
//!   sender on that stream is dropped; the same seq with different content is
//!   a sender fork. DMs use stream [`DM_STREAM`]; later phases pick their own
//!   stream names (e.g. per server channel).
//! * Olm needs nothing extra: a replayed Olm message fails to decrypt, since
//!   its message key is gone.

use sha2::{Digest, Sha256};

use crate::engine::proto::Payload;

/// Inbound stream for DMs (the sender is the peer).
pub const DM_STREAM: &str = "dm";

/// How far our sequence numbers jump after a restore or when a peer reports
/// that it saw higher ones from us. Far beyond any real gap.
pub const SEQ_RESTORE_GAP: u64 = 1 << 20;

/// Our outgoing DM stream to `peer`.
pub fn dm_out_stream(peer: &str) -> String {
    format!("dm:{peer}")
}

/// Content digest for fork detection: SHA-256 over the id, timestamp and
/// body (not the envelope fields, which a jump may change), truncated to
/// 16 bytes. Stable across retransmissions of the same message.
pub fn payload_digest(p: &Payload) -> Vec<u8> {
    let mut h = Sha256::new();
    h.update(serde_json::to_vec(&(&p.id, p.ts, &p.body)).unwrap_or_default());
    h.finalize()[..16].to_vec()
}
