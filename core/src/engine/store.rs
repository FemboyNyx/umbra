//! SQLCipher-encrypted local store.
//!
//! The 32-byte key comes from the app (OS keystore) and is applied as a raw
//! SQLCipher key; it is never written anywhere by core.

use std::path::Path;
use std::sync::{Mutex, MutexGuard};

use anyhow::{anyhow, bail, Context, Result};
use rusqlite::{params, Connection, OptionalExtension};
use zeroize::Zeroizing;

use crate::api::types::{Contact, ConversationSummary, DeliveryStatus, Message};
use crate::engine::proto::EnvelopeMeta;

const SCHEMA_VERSION: i64 = 5;

const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS meta (
    key   TEXT PRIMARY KEY,
    value BLOB NOT NULL
);
CREATE TABLE IF NOT EXISTS contacts (
    peer_id      TEXT PRIMARY KEY,
    display_name TEXT NOT NULL DEFAULT '',
    identity_key TEXT NOT NULL,
    ed25519_key  TEXT NOT NULL,
    addrs        TEXT NOT NULL DEFAULT '[]',
    verified     INTEGER NOT NULL DEFAULT 0,
    added_at     INTEGER NOT NULL
);
CREATE TABLE IF NOT EXISTS olm_sessions (
    session_id TEXT PRIMARY KEY,
    peer_id    TEXT NOT NULL,
    pickle     TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    last_used  INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS olm_sessions_peer ON olm_sessions(peer_id, last_used);
CREATE TABLE IF NOT EXISTS messages (
    id       TEXT PRIMARY KEY,
    peer_id  TEXT NOT NULL,
    outgoing INTEGER NOT NULL,
    body     TEXT NOT NULL,
    ts       INTEGER NOT NULL,
    status   INTEGER NOT NULL,
    read     INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX IF NOT EXISTS messages_peer_ts ON messages(peer_id, ts);
CREATE TABLE IF NOT EXISTS outbox (
    msg_id       TEXT PRIMARY KEY,
    peer_id      TEXT NOT NULL,
    created_at   INTEGER NOT NULL,
    attempts     INTEGER NOT NULL DEFAULT 0,
    last_attempt INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX IF NOT EXISTS outbox_peer ON outbox(peer_id, created_at);
-- v3: servers Phase 0 (docs/servers.md). Which of OUR identities a server
-- membership uses, and per-server settings that later phases read.
CREATE TABLE IF NOT EXISTS server_memberships (
    server_id          TEXT PRIMARY KEY,
    identity           TEXT NOT NULL,   -- 'default' | 'dedicated'
    history_visibility TEXT,            -- NULL = not set (policy decides)
    joined_at          INTEGER NOT NULL
);
-- Co-members: peers we share a server with. NOT contacts. One row per
-- (server, member identity); the member identity is their PeerId, which is
-- derived from (and so equivalent to) their Ed25519 key.
CREATE TABLE IF NOT EXISTS co_members (
    server_id             TEXT NOT NULL,
    peer_id               TEXT NOT NULL,
    display_name          TEXT NOT NULL DEFAULT '',
    identity_key          TEXT NOT NULL,
    ed25519_key           TEXT NOT NULL,
    card_sig              TEXT,
    addrs                 TEXT NOT NULL DEFAULT '[]',
    added_at              INTEGER NOT NULL,
    pending_identity_key  TEXT,
    pending_card_sig      TEXT,
    rejected_identity_key TEXT,
    PRIMARY KEY (server_id, peer_id)
);
CREATE INDEX IF NOT EXISTS co_members_peer ON co_members(peer_id);
-- v4: anti-replay (docs/message-protection.md#anti-replay).
-- Every (Megolm session, message index) accepted, until the session goes.
CREATE TABLE IF NOT EXISTS group_seen (
    session_id TEXT NOT NULL,
    msg_index  INTEGER NOT NULL,
    PRIMARY KEY (session_id, msg_index)
) WITHOUT ROWID;
-- Recent (stream, sender, seq) accepted, with a digest of the content so a
-- reused seq with different content shows up as a fork. The highest seq per
-- (stream, sender) is always kept.
CREATE TABLE IF NOT EXISTS seq_seen (
    stream TEXT NOT NULL,
    sender TEXT NOT NULL,
    seq    INTEGER NOT NULL,
    digest BLOB NOT NULL,
    PRIMARY KEY (stream, sender, seq)
) WITHOUT ROWID;
-- Our own next outgoing seq per stream (e.g. 'dm:<peer>').
CREATE TABLE IF NOT EXISTS seq_out (
    stream TEXT PRIMARY KEY,
    next   INTEGER NOT NULL
);
-- v5: servers Phase 1, server model (docs/servers.md "Membership, roles and
-- moderation"). Signed manifests and roster ops are the source of truth and
-- are kept as the exact signed bytes; `server_channels` / `server_roles` are
-- the effective state, rebuilt whenever it changes.
CREATE TABLE IF NOT EXISTS servers (
    server_id  TEXT PRIMARY KEY,
    owner_key  BLOB,                -- K_S secret, only on the owner's device
    anchor_key TEXT NOT NULL,       -- owner key we first trusted (genesis or invite)
    frozen_seq INTEGER,             -- set when a fork was seen: read-only
    removed    TEXT,                -- 'kicked' | 'banned' | 'left'
    created_at INTEGER NOT NULL
);
CREATE TABLE IF NOT EXISTS manifests (
    server_id   TEXT NOT NULL,
    seq         INTEGER NOT NULL,
    hash        TEXT NOT NULL,
    bytes       BLOB NOT NULL,
    sig         BLOB NOT NULL,
    received_at INTEGER NOT NULL,
    PRIMARY KEY (server_id, seq)
);
-- Conflicting manifests for a seq we already have (evidence of a fork).
CREATE TABLE IF NOT EXISTS manifest_forks (
    server_id TEXT NOT NULL,
    hash      TEXT NOT NULL,
    seq       INTEGER NOT NULL,
    bytes     BLOB NOT NULL,
    sig       BLOB NOT NULL,
    PRIMARY KEY (server_id, hash)
);
CREATE TABLE IF NOT EXISTS roster_ops (
    server_id   TEXT NOT NULL,
    hash        TEXT NOT NULL,
    base_seq    INTEGER NOT NULL,
    bytes       BLOB NOT NULL,
    sig         BLOB NOT NULL,
    received_at INTEGER NOT NULL,
    PRIMARY KEY (server_id, hash)
);
CREATE TABLE IF NOT EXISTS server_channels (
    server_id  TEXT NOT NULL,
    channel_id TEXT NOT NULL,
    position   INTEGER NOT NULL,
    channel    BLOB NOT NULL,       -- CBOR server_model::Channel
    PRIMARY KEY (server_id, channel_id)
);
CREATE TABLE IF NOT EXISTS server_roles (
    server_id TEXT NOT NULL,
    peer_id   TEXT NOT NULL,
    role      TEXT NOT NULL,        -- 'owner' | 'admin' | 'member'
    PRIMARY KEY (server_id, peer_id)
);
-- Invites we created (the secret never leaves this device except in the link).
CREATE TABLE IF NOT EXISTS server_invites (
    invite_id      TEXT PRIMARY KEY,
    server_id      TEXT NOT NULL,
    secret         BLOB NOT NULL,
    signed         BLOB NOT NULL,   -- CBOR SignedInvite
    created_at     INTEGER NOT NULL,
    expires_at     INTEGER NOT NULL,
    max_uses       INTEGER NOT NULL,
    uses           INTEGER NOT NULL DEFAULT 0,
    needs_approval INTEGER NOT NULL,
    revoked        INTEGER NOT NULL DEFAULT 0
);
-- JoinHellos waiting for approval (invites with needs_approval).
CREATE TABLE IF NOT EXISTS join_requests (
    server_id TEXT NOT NULL,
    peer_id   TEXT NOT NULL,
    invite_id TEXT NOT NULL,
    card      TEXT NOT NULL,        -- JSON ContactCard
    ts        INTEGER NOT NULL,
    PRIMARY KEY (server_id, peer_id)
);
-- Servers we are joining: the signed invite until the first manifest arrives.
CREATE TABLE IF NOT EXISTS pending_joins (
    server_id TEXT PRIMARY KEY,
    invite    BLOB NOT NULL,        -- CBOR SignedInvite
    ts        INTEGER NOT NULL
);
"#;

pub struct Store {
    conn: Mutex<Connection>,
}

#[path = "store_servers.rs"]
mod store_servers;
pub use store_servers::*;

/// Outcome of checking a `(stream, sender, seq)` against what was accepted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SeqVerdict {
    /// Above everything seen: accept (and [`Store::seq_commit`] it).
    Fresh,
    /// Same seq, same content: a retransmission. Ack, don't deliver again.
    Duplicate,
    /// Same seq, different content: the sender forked (or was rolled back).
    Fork,
    /// At or below the highest seq seen and not a known duplicate: a replay.
    Stale,
}

/// `seq_seen` rows kept per (stream, sender) besides the highest.
pub const SEQ_SEEN_KEEP: i64 = 1024;
const META_SEQ_FLOOR: &str = "seq_floor";

#[derive(Debug, Clone)]
pub struct SessionRow {
    pub session_id: String,
    pub pickle: String,
}

/// What an outbox row carries.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OutboxItem {
    /// A DM: the text lives in `messages` (`msg_id`).
    Dm,
    /// A server-scoped payload: `payload` is the full Olm plaintext
    /// (`Payload` JSON) for co-member `peer_id`.
    Server { server_id: String, payload: Vec<u8> },
}

#[derive(Debug, Clone)]
pub struct OutboxRow {
    pub msg_id: String,
    pub peer_id: String,
    pub created_at: i64,
    pub attempts: i64,
    pub item: OutboxItem,
    pub meta: EnvelopeMeta,
    /// Which of our identities sends it (`"default"`; see `ServerIdentity`).
    pub local_identity: String,
}

const OUTBOX_COLS: &str =
    "msg_id, peer_id, created_at, attempts, kind, server_id, payload, seq, prev_hash, expires_at, local_identity";

fn row_to_outbox(r: &rusqlite::Row<'_>) -> rusqlite::Result<OutboxRow> {
    let kind: String = r.get(4)?;
    let item = match kind.as_str() {
        "server" => OutboxItem::Server {
            server_id: r.get::<_, Option<String>>(5)?.unwrap_or_default(),
            payload: r.get::<_, Option<Vec<u8>>>(6)?.unwrap_or_default(),
        },
        _ => OutboxItem::Dm,
    };
    Ok(OutboxRow {
        msg_id: r.get(0)?,
        peer_id: r.get(1)?,
        created_at: r.get(2)?,
        attempts: r.get(3)?,
        item,
        meta: EnvelopeMeta {
            seq: r.get::<_, Option<i64>>(7)?.map(|v| v as u64),
            prev: r.get(8)?,
            expires_at: r.get(9)?,
        },
        local_identity: r.get(10)?,
    })
}

fn status_to_i(s: DeliveryStatus) -> i64 {
    match s {
        DeliveryStatus::Sent => 0,
        DeliveryStatus::Delivered => 1,
        DeliveryStatus::Failed => 2,
        DeliveryStatus::Received => 3,
    }
}

fn status_from_i(i: i64) -> DeliveryStatus {
    match i {
        1 => DeliveryStatus::Delivered,
        2 => DeliveryStatus::Failed,
        3 => DeliveryStatus::Received,
        _ => DeliveryStatus::Sent,
    }
}

fn row_to_message(r: &rusqlite::Row<'_>) -> rusqlite::Result<Message> {
    Ok(Message {
        id: r.get(0)?,
        peer_id: r.get(1)?,
        outgoing: r.get::<_, i64>(2)? != 0,
        body: r.get(3)?,
        timestamp_ms: r.get(4)?,
        status: status_from_i(r.get(5)?),
        read: r.get::<_, i64>(6)? != 0,
    })
}

const MSG_COLS: &str = "id, peer_id, outgoing, body, ts, status, read";

/// One co-member record (`server_id`, `peer_id`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoMemberRow {
    pub server_id: String,
    pub peer_id: String,
    pub display_name: String,
    pub identity_key: String,
    pub ed25519_key: String,
    /// Card v2 signature over (`peer_id`, `identity_key`). Required to add a
    /// co-member; `None` only after accepting an unsigned key change, which
    /// blocks server traffic until a signed `Hello` restores it.
    pub card_sig: Option<String>,
    pub addrs: Vec<String>,
    pub added_at: i64,
    pub pending_identity_key: Option<String>,
    pub pending_card_sig: Option<String>,
    pub rejected_identity_key: Option<String>,
}

const CO_MEMBER_COLS: &str = "server_id, peer_id, display_name, identity_key, ed25519_key, card_sig, addrs, added_at, pending_identity_key, pending_card_sig, rejected_identity_key";

fn row_to_co_member(r: &rusqlite::Row<'_>) -> rusqlite::Result<CoMemberRow> {
    let addrs: String = r.get(6)?;
    Ok(CoMemberRow {
        server_id: r.get(0)?,
        peer_id: r.get(1)?,
        display_name: r.get(2)?,
        identity_key: r.get(3)?,
        ed25519_key: r.get(4)?,
        card_sig: r.get(5)?,
        addrs: serde_json::from_str(&addrs).unwrap_or_default(),
        added_at: r.get(7)?,
        pending_identity_key: r.get(8)?,
        pending_card_sig: r.get(9)?,
        rejected_identity_key: r.get(10)?,
    })
}

/// Local server membership record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MembershipRow {
    pub server_id: String,
    /// `"default"` or `"dedicated"`.
    pub identity: String,
    pub history_visibility: Option<String>,
    pub joined_at: i64,
}

/// Contact row without the derived fields (safety number, online).
#[derive(Debug, Clone)]
pub struct ContactRow {
    pub peer_id: String,
    pub display_name: String,
    pub identity_key: String,
    pub ed25519_key: String,
    pub addrs: Vec<String>,
    pub verified: bool,
    pub added_at: i64,
    /// Different identity key presented by this contact, awaiting a user decision.
    pub pending_identity_key: Option<String>,
    /// Identity key the user refused; presenting it again is ignored silently.
    pub rejected_identity_key: Option<String>,
    /// Card v2 signature over (`peer_id`, `identity_key`); `None` = legacy.
    pub card_sig: Option<String>,
    /// Card v2 signature that came with `pending_identity_key`, if any.
    pub pending_card_sig: Option<String>,
}

impl ContactRow {
    pub fn into_contact(self, safety_number: String, online: bool) -> Contact {
        Contact {
            peer_id: self.peer_id,
            display_name: self.display_name,
            identity_key: self.identity_key,
            ed25519_key: self.ed25519_key,
            addrs: self.addrs,
            verified: self.verified,
            safety_number,
            added_at_ms: self.added_at,
            online,
            key_changed: self.pending_identity_key.is_some(),
            pending_identity_key: self.pending_identity_key,
            signed_card: self.card_sig.is_some(),
        }
    }
}

fn row_to_contact(r: &rusqlite::Row<'_>) -> rusqlite::Result<ContactRow> {
    let addrs: String = r.get(4)?;
    Ok(ContactRow {
        peer_id: r.get(0)?,
        display_name: r.get(1)?,
        identity_key: r.get(2)?,
        ed25519_key: r.get(3)?,
        addrs: serde_json::from_str(&addrs).unwrap_or_default(),
        verified: r.get::<_, i64>(5)? != 0,
        added_at: r.get(6)?,
        pending_identity_key: r.get(7)?,
        rejected_identity_key: r.get(8)?,
        card_sig: r.get(9)?,
        pending_card_sig: r.get(10)?,
    })
}

const CONTACT_COLS: &str =
    "peer_id, display_name, identity_key, ed25519_key, addrs, verified, added_at, pending_identity_key, rejected_identity_key, card_sig, pending_card_sig";

impl Store {
    /// Open or create `path`, keyed with the raw 32-byte `key`.
    pub fn open(path: &Path, key: &[u8]) -> Result<Self> {
        if key.len() != 32 {
            bail!("db_key must be 32 bytes");
        }
        let conn = Connection::open(path).with_context(|| format!("open {}", path.display()))?;
        Self::init_conn(conn, key)
    }

    /// In-memory encrypted DB (tests).
    pub fn open_in_memory(key: &[u8]) -> Result<Self> {
        Self::init_conn(Connection::open_in_memory()?, key)
    }

    fn init_conn(conn: Connection, key: &[u8]) -> Result<Self> {
        let pragma = Zeroizing::new(format!("PRAGMA key = \"x'{}'\";", hex::encode(key)));
        conn.execute_batch(&pragma)?;
        // Fails with "file is not a database" on a wrong key.
        conn.query_row("SELECT count(*) FROM sqlite_master", [], |r| {
            r.get::<_, i64>(0)
        })
        .map_err(|_| anyhow!("cannot open store: wrong key or corrupted database"))?;
        let cipher: Option<String> = conn
            .query_row("PRAGMA cipher_version", [], |r| r.get(0))
            .optional()?;
        if cipher.is_none() {
            bail!("SQLCipher not available; refusing to store data unencrypted");
        }
        conn.execute_batch(
            "PRAGMA foreign_keys = ON; PRAGMA journal_mode = WAL; PRAGMA secure_delete = ON;",
        )?;
        let mut conn = conn;
        migrate(&mut conn)?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    #[cfg(test)]
    pub(crate) fn conn_for_test(&self) -> MutexGuard<'_, Connection> {
        self.c()
    }

    fn c(&self) -> MutexGuard<'_, Connection> {
        self.conn.lock().expect("store poisoned")
    }

    // ---- meta ----

    pub fn meta_get(&self, key: &str) -> Result<Option<Vec<u8>>> {
        Ok(self
            .c()
            .query_row("SELECT value FROM meta WHERE key = ?1", [key], |r| r.get(0))
            .optional()?)
    }

    pub fn meta_set(&self, key: &str, value: &[u8]) -> Result<()> {
        self.c().execute(
            "INSERT INTO meta(key, value) VALUES (?1, ?2) ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![key, value],
        )?;
        Ok(())
    }

    #[cfg(test)]
    fn meta_get_i64(&self, key: &str) -> Result<Option<i64>> {
        Ok(self
            .meta_get(key)?
            .and_then(|v| String::from_utf8(v).ok())
            .and_then(|s| s.parse().ok()))
    }

    // ---- contacts ----

    pub fn upsert_contact(&self, c: &ContactRow) -> Result<()> {
        self.c().execute(
            "INSERT INTO contacts(peer_id, display_name, identity_key, ed25519_key, addrs, verified, added_at, card_sig)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
             ON CONFLICT(peer_id) DO UPDATE SET display_name = excluded.display_name,
               addrs = excluded.addrs, verified = excluded.verified,
               card_sig = COALESCE(excluded.card_sig, contacts.card_sig)",
            params![
                c.peer_id,
                c.display_name,
                c.identity_key,
                c.ed25519_key,
                serde_json::to_string(&c.addrs)?,
                c.verified as i64,
                c.added_at,
                c.card_sig
            ],
        )?;
        Ok(())
    }

    pub fn get_contact(&self, peer_id: &str) -> Result<Option<ContactRow>> {
        Ok(self
            .c()
            .query_row(
                &format!("SELECT {CONTACT_COLS} FROM contacts WHERE peer_id = ?1"),
                [peer_id],
                row_to_contact,
            )
            .optional()?)
    }

    pub fn list_contacts(&self) -> Result<Vec<ContactRow>> {
        let c = self.c();
        let mut st = c.prepare(&format!(
            "SELECT {CONTACT_COLS} FROM contacts ORDER BY display_name, peer_id"
        ))?;
        let rows = st
            .query_map([], row_to_contact)?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }

    pub fn set_verified(&self, peer_id: &str, v: bool) -> Result<()> {
        let n = self.c().execute(
            "UPDATE contacts SET verified = ?2 WHERE peer_id = ?1",
            params![peer_id, v as i64],
        )?;
        if n == 0 {
            bail!("unknown contact {peer_id}");
        }
        Ok(())
    }

    pub fn update_contact_seen(
        &self,
        peer_id: &str,
        name: Option<&str>,
        addrs: Option<&[String]>,
    ) -> Result<()> {
        let c = self.c();
        if let Some(name) = name {
            c.execute(
                "UPDATE contacts SET display_name = ?2 WHERE peer_id = ?1 AND display_name = ''",
                params![peer_id, name],
            )?;
        }
        if let Some(addrs) = addrs {
            c.execute(
                "UPDATE contacts SET addrs = ?2 WHERE peer_id = ?1",
                params![peer_id, serde_json::to_string(addrs)?],
            )?;
        }
        Ok(())
    }

    /// Flag a key change (keeps the trusted key in place). `sig` is the card
    /// v2 signature that came with the new key, if any.
    pub fn set_pending_key(
        &self,
        peer_id: &str,
        key: &str,
        sig: Option<&str>,
        now: i64,
    ) -> Result<()> {
        self.c().execute(
            "UPDATE contacts SET pending_identity_key = ?2, pending_card_sig = ?3, key_changed_at = ?4
             WHERE peer_id = ?1",
            params![peer_id, key, sig, now],
        )?;
        Ok(())
    }

    /// Record a (verified) card v2 signature for the trusted key.
    pub fn set_card_sig(&self, peer_id: &str, sig: &str) -> Result<()> {
        self.c().execute(
            "UPDATE contacts SET card_sig = ?2 WHERE peer_id = ?1",
            params![peer_id, sig],
        )?;
        Ok(())
    }

    /// Trust the pending key: it replaces the identity key and the contact
    /// becomes unverified.
    pub fn accept_pending_key(&self, peer_id: &str) -> Result<()> {
        let n = self.c().execute(
            "UPDATE contacts SET identity_key = pending_identity_key, pending_identity_key = NULL,
               card_sig = pending_card_sig, pending_card_sig = NULL,
               rejected_identity_key = NULL, key_changed_at = NULL, verified = 0
             WHERE peer_id = ?1 AND pending_identity_key IS NOT NULL",
            [peer_id],
        )?;
        if n == 0 {
            bail!("no pending key change for {peer_id}");
        }
        Ok(())
    }

    /// Refuse the pending key and remember it so it doesn't re-alert.
    pub fn reject_pending_key(&self, peer_id: &str) -> Result<()> {
        let n = self.c().execute(
            "UPDATE contacts SET rejected_identity_key = pending_identity_key,
               pending_identity_key = NULL, pending_card_sig = NULL, key_changed_at = NULL
             WHERE peer_id = ?1 AND pending_identity_key IS NOT NULL",
            [peer_id],
        )?;
        if n == 0 {
            bail!("no pending key change for {peer_id}");
        }
        Ok(())
    }

    /// Delete contact plus its messages and outbox entries, and its Olm
    /// sessions unless `keep_sessions` (the peer is still a co-member).
    pub fn remove_contact(&self, peer_id: &str, keep_sessions: bool) -> Result<()> {
        let mut c = self.c();
        let tx = c.transaction()?;
        let n = tx.execute("DELETE FROM contacts WHERE peer_id = ?1", [peer_id])?;
        if !keep_sessions {
            tx.execute("DELETE FROM olm_sessions WHERE peer_id = ?1", [peer_id])?;
        }
        tx.execute("DELETE FROM messages WHERE peer_id = ?1", [peer_id])?;
        tx.execute(
            "DELETE FROM outbox WHERE peer_id = ?1 AND kind = 'dm'",
            [peer_id],
        )?;
        tx.commit()?;
        if n == 0 {
            bail!("unknown contact {peer_id}");
        }
        Ok(())
    }

    // ---- servers / co-members ----

    pub fn upsert_membership(&self, m: &MembershipRow) -> Result<()> {
        self.c().execute(
            "INSERT INTO server_memberships(server_id, identity, history_visibility, joined_at)
             VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(server_id) DO UPDATE SET history_visibility = excluded.history_visibility",
            params![m.server_id, m.identity, m.history_visibility, m.joined_at],
        )?;
        Ok(())
    }

    pub fn get_membership(&self, server_id: &str) -> Result<Option<MembershipRow>> {
        Ok(self
            .c()
            .query_row(
                "SELECT server_id, identity, history_visibility, joined_at FROM server_memberships WHERE server_id = ?1",
                [server_id],
                |r| {
                    Ok(MembershipRow {
                        server_id: r.get(0)?,
                        identity: r.get(1)?,
                        history_visibility: r.get(2)?,
                        joined_at: r.get(3)?,
                    })
                },
            )
            .optional()?)
    }

    /// Insert or refresh a co-member. The pinned key and key-change state are
    /// never overwritten here (callers compare keys first).
    pub fn upsert_co_member(&self, m: &CoMemberRow) -> Result<()> {
        self.c().execute(
            "INSERT INTO co_members(server_id, peer_id, display_name, identity_key, ed25519_key, card_sig, addrs, added_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
             ON CONFLICT(server_id, peer_id) DO UPDATE SET display_name = excluded.display_name,
               addrs = excluded.addrs, card_sig = COALESCE(co_members.card_sig, excluded.card_sig)",
            params![
                m.server_id,
                m.peer_id,
                m.display_name,
                m.identity_key,
                m.ed25519_key,
                m.card_sig,
                serde_json::to_string(&m.addrs)?,
                m.added_at
            ],
        )?;
        Ok(())
    }

    /// All co-member records of one peer (one per shared server).
    pub fn co_member_rows(&self, peer_id: &str) -> Result<Vec<CoMemberRow>> {
        let c = self.c();
        let mut st = c.prepare(&format!(
            "SELECT {CO_MEMBER_COLS} FROM co_members WHERE peer_id = ?1 ORDER BY server_id"
        ))?;
        let rows = st
            .query_map([peer_id], row_to_co_member)?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }

    pub fn list_co_members(&self, server_id: Option<&str>) -> Result<Vec<CoMemberRow>> {
        let c = self.c();
        let mut st = c.prepare(&format!(
            "SELECT {CO_MEMBER_COLS} FROM co_members WHERE ?1 IS NULL OR server_id = ?1
             ORDER BY server_id, display_name, peer_id"
        ))?;
        let rows = st
            .query_map([server_id], row_to_co_member)?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }

    /// Remove a co-member record and its queued server payloads for that
    /// server. Returns false if there was no such record.
    pub fn remove_co_member(&self, server_id: &str, peer_id: &str) -> Result<bool> {
        let mut c = self.c();
        let tx = c.transaction()?;
        let n = tx.execute(
            "DELETE FROM co_members WHERE server_id = ?1 AND peer_id = ?2",
            params![server_id, peer_id],
        )?;
        tx.execute(
            "DELETE FROM outbox WHERE kind = 'server' AND server_id = ?1 AND peer_id = ?2",
            params![server_id, peer_id],
        )?;
        tx.commit()?;
        Ok(n > 0)
    }

    pub fn co_member_seen(&self, peer_id: &str, addrs: &[String], sig: Option<&str>) -> Result<()> {
        self.c().execute(
            "UPDATE co_members SET addrs = ?2, card_sig = COALESCE(card_sig, ?3) WHERE peer_id = ?1",
            params![peer_id, serde_json::to_string(addrs)?, sig],
        )?;
        Ok(())
    }

    /// Flag a key change on every record of `peer_id`.
    pub fn co_member_set_pending(&self, peer_id: &str, key: &str, sig: &str) -> Result<()> {
        self.c().execute(
            "UPDATE co_members SET pending_identity_key = ?2, pending_card_sig = ?3 WHERE peer_id = ?1",
            params![peer_id, key, sig],
        )?;
        Ok(())
    }

    /// Pin `key` (with `sig`, if any) on every record of `peer_id` and clear
    /// key-change state.
    pub fn co_member_set_key(&self, peer_id: &str, key: &str, sig: Option<&str>) -> Result<()> {
        self.c().execute(
            "UPDATE co_members SET identity_key = ?2, card_sig = ?3, pending_identity_key = NULL,
               pending_card_sig = NULL, rejected_identity_key = NULL WHERE peer_id = ?1",
            params![peer_id, key, sig],
        )?;
        Ok(())
    }

    pub fn co_member_reject(&self, peer_id: &str) -> Result<()> {
        self.c().execute(
            "UPDATE co_members SET rejected_identity_key = pending_identity_key,
               pending_identity_key = NULL, pending_card_sig = NULL
             WHERE peer_id = ?1 AND pending_identity_key IS NOT NULL",
            [peer_id],
        )?;
        Ok(())
    }

    // ---- olm sessions ----

    /// Sessions for a peer, most recently used first.
    pub fn sessions_for(&self, peer_id: &str) -> Result<Vec<SessionRow>> {
        let c = self.c();
        let mut st = c.prepare(
            "SELECT session_id, pickle FROM olm_sessions WHERE peer_id = ?1 ORDER BY last_used DESC, created_at DESC",
        )?;
        let rows = st
            .query_map([peer_id], |r| {
                Ok(SessionRow {
                    session_id: r.get(0)?,
                    pickle: r.get(1)?,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }

    pub fn save_session(
        &self,
        peer_id: &str,
        session_id: &str,
        pickle: &str,
        now: i64,
    ) -> Result<()> {
        self.c().execute(
            "INSERT INTO olm_sessions(session_id, peer_id, pickle, created_at, last_used) VALUES (?1, ?2, ?3, ?4, ?4)
             ON CONFLICT(session_id) DO UPDATE SET pickle = excluded.pickle, last_used = excluded.last_used",
            params![session_id, peer_id, pickle, now],
        )?;
        Ok(())
    }

    /// Drop every Olm session (after a restore).
    pub fn delete_all_sessions(&self) -> Result<usize> {
        Ok(self.c().execute("DELETE FROM olm_sessions", [])?)
    }

    pub fn delete_sessions(&self, peer_id: &str) -> Result<()> {
        self.c()
            .execute("DELETE FROM olm_sessions WHERE peer_id = ?1", [peer_id])?;
        Ok(())
    }

    /// Keep at most `keep` sessions per peer (most recently used).
    pub fn prune_sessions(&self, peer_id: &str, keep: usize) -> Result<()> {
        self.c().execute(
            "DELETE FROM olm_sessions WHERE peer_id = ?1 AND session_id NOT IN
               (SELECT session_id FROM olm_sessions WHERE peer_id = ?1 ORDER BY last_used DESC LIMIT ?2)",
            params![peer_id, keep as i64],
        )?;
        Ok(())
    }

    // ---- messages ----

    /// Returns false if a message with this id already existed (dedupe).
    pub fn insert_message(&self, m: &Message) -> Result<bool> {
        self.insert_message_with_meta(m, &EnvelopeMeta::default())
    }

    /// [`Self::insert_message`] storing the optional envelope fields too.
    pub fn insert_message_with_meta(&self, m: &Message, meta: &EnvelopeMeta) -> Result<bool> {
        let n = self.c().execute(
            "INSERT OR IGNORE INTO messages(id, peer_id, outgoing, body, ts, status, read, seq, prev_hash, expires_at)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)",
            params![
                m.id,
                m.peer_id,
                m.outgoing as i64,
                m.body,
                m.timestamp_ms,
                status_to_i(m.status),
                m.read as i64,
                meta.seq.map(|v| v as i64),
                meta.prev,
                meta.expires_at
            ],
        )?;
        Ok(n == 1)
    }

    /// The optional envelope fields stored with a message.
    pub fn message_meta(&self, id: &str) -> Result<Option<EnvelopeMeta>> {
        Ok(self
            .c()
            .query_row(
                "SELECT seq, prev_hash, expires_at FROM messages WHERE id = ?1",
                [id],
                |r| {
                    Ok(EnvelopeMeta {
                        seq: r.get::<_, Option<i64>>(0)?.map(|v| v as u64),
                        prev: r.get(1)?,
                        expires_at: r.get(2)?,
                    })
                },
            )
            .optional()?)
    }

    pub fn get_message(&self, id: &str) -> Result<Option<Message>> {
        Ok(self
            .c()
            .query_row(
                &format!("SELECT {MSG_COLS} FROM messages WHERE id = ?1"),
                [id],
                row_to_message,
            )
            .optional()?)
    }

    pub fn set_message_status(&self, id: &str, s: DeliveryStatus) -> Result<()> {
        self.c().execute(
            "UPDATE messages SET status = ?2 WHERE id = ?1",
            params![id, status_to_i(s)],
        )?;
        Ok(())
    }

    pub fn list_messages(
        &self,
        peer_id: &str,
        before: Option<i64>,
        limit: u32,
    ) -> Result<Vec<Message>> {
        let c = self.c();
        let mut st = c.prepare(&format!(
            "SELECT {MSG_COLS} FROM messages WHERE peer_id = ?1 AND ts < ?2 ORDER BY ts DESC, rowid DESC LIMIT ?3"
        ))?;
        let rows = st
            .query_map(
                params![peer_id, before.unwrap_or(i64::MAX), limit as i64],
                row_to_message,
            )?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }

    pub fn mark_read(&self, peer_id: &str, up_to: &str) -> Result<()> {
        let c = self.c();
        let ts: i64 = c
            .query_row(
                "SELECT ts FROM messages WHERE id = ?1 AND peer_id = ?2",
                params![up_to, peer_id],
                |r| r.get(0),
            )
            .optional()?
            .ok_or_else(|| anyhow!("unknown message {up_to}"))?;
        c.execute(
            "UPDATE messages SET read = 1 WHERE peer_id = ?1 AND outgoing = 0 AND ts <= ?2",
            params![peer_id, ts],
        )?;
        Ok(())
    }

    pub fn list_conversations(&self) -> Result<Vec<ConversationSummary>> {
        let contacts = self.list_contacts()?;
        let c = self.c();
        let mut last = c.prepare(&format!(
            "SELECT {MSG_COLS} FROM messages WHERE peer_id = ?1 ORDER BY ts DESC, rowid DESC LIMIT 1"
        ))?;
        let mut unread = c.prepare(
            "SELECT count(*) FROM messages WHERE peer_id = ?1 AND outgoing = 0 AND read = 0",
        )?;
        let mut out = Vec::with_capacity(contacts.len());
        for ct in contacts {
            let last_message = last.query_row([&ct.peer_id], row_to_message).optional()?;
            let unread_count: i64 = unread.query_row([&ct.peer_id], |r| r.get(0))?;
            out.push(ConversationSummary {
                updated_at_ms: last_message
                    .as_ref()
                    .map(|m| m.timestamp_ms)
                    .unwrap_or(ct.added_at),
                peer_id: ct.peer_id,
                display_name: ct.display_name,
                last_message,
                unread_count: unread_count as u32,
            });
        }
        out.sort_by_key(|c| std::cmp::Reverse(c.updated_at_ms));
        Ok(out)
    }

    // ---- anti-replay ----

    /// Record a Megolm `(session_id, index)`. `false` if it was already
    /// accepted (a replay).
    pub fn group_seen_record(&self, session_id: &str, index: u32) -> Result<bool> {
        Ok(self.c().execute(
            "INSERT OR IGNORE INTO group_seen(session_id, msg_index) VALUES (?1, ?2)",
            params![session_id, index],
        )? == 1)
    }

    /// Drop the replay record of a discarded group session.
    pub fn group_seen_forget(&self, session_id: &str) -> Result<()> {
        self.c()
            .execute("DELETE FROM group_seen WHERE session_id = ?1", [session_id])?;
        Ok(())
    }

    #[cfg(test)]
    pub fn group_seen_count(&self, session_id: &str) -> Result<i64> {
        Ok(self.c().query_row(
            "SELECT COUNT(*) FROM group_seen WHERE session_id = ?1",
            [session_id],
            |r| r.get(0),
        )?)
    }

    /// Check a `(stream, sender, seq)` without recording it.
    pub fn seq_check(
        &self,
        stream: &str,
        sender: &str,
        seq: u64,
        digest: &[u8],
    ) -> Result<SeqVerdict> {
        let c = self.c();
        let seq = i64::try_from(seq).map_err(|_| anyhow!("seq out of range"))?;
        let same: Option<Vec<u8>> = c
            .query_row(
                "SELECT digest FROM seq_seen WHERE stream = ?1 AND sender = ?2 AND seq = ?3",
                params![stream, sender, seq],
                |r| r.get(0),
            )
            .optional()?;
        if let Some(d) = same {
            return Ok(if d == digest {
                SeqVerdict::Duplicate
            } else {
                SeqVerdict::Fork
            });
        }
        let max: Option<i64> = c.query_row(
            "SELECT MAX(seq) FROM seq_seen WHERE stream = ?1 AND sender = ?2",
            params![stream, sender],
            |r| r.get(0),
        )?;
        Ok(match max {
            Some(m) if seq <= m => SeqVerdict::Stale,
            _ => SeqVerdict::Fresh,
        })
    }

    /// Record an accepted `(stream, sender, seq)`, keeping the most recent
    /// [`SEQ_SEEN_KEEP`] per (stream, sender).
    pub fn seq_commit(&self, stream: &str, sender: &str, seq: u64, digest: &[u8]) -> Result<()> {
        let seq = i64::try_from(seq).map_err(|_| anyhow!("seq out of range"))?;
        let mut c = self.c();
        let tx = c.transaction()?;
        tx.execute(
            "INSERT OR IGNORE INTO seq_seen(stream, sender, seq, digest) VALUES (?1, ?2, ?3, ?4)",
            params![stream, sender, seq, digest],
        )?;
        tx.execute(
            "DELETE FROM seq_seen WHERE stream = ?1 AND sender = ?2 AND seq NOT IN
               (SELECT seq FROM seq_seen WHERE stream = ?1 AND sender = ?2 ORDER BY seq DESC LIMIT ?3)",
            params![stream, sender, SEQ_SEEN_KEEP],
        )?;
        tx.commit()?;
        Ok(())
    }

    /// Drop every seq record of `stream` (e.g. a server we left).
    pub fn seq_forget_stream(&self, stream: &str) -> Result<()> {
        self.c()
            .execute("DELETE FROM seq_seen WHERE stream = ?1", [stream])?;
        Ok(())
    }

    /// Take our next outgoing seq on `stream`. Starts at 1, and never below
    /// the floor set by [`Self::seq_jump_all`].
    pub fn seq_next(&self, stream: &str) -> Result<u64> {
        let mut c = self.c();
        let tx = c.transaction()?;
        let floor = meta_i64(&tx, META_SEQ_FLOOR)?.unwrap_or(0);
        let cur: Option<i64> = tx
            .query_row(
                "SELECT next FROM seq_out WHERE stream = ?1",
                [stream],
                |r| r.get(0),
            )
            .optional()?;
        let n = cur.unwrap_or(1).max(floor + 1).max(1);
        tx.execute(
            "INSERT INTO seq_out(stream, next) VALUES (?1, ?2)
             ON CONFLICT(stream) DO UPDATE SET next = excluded.next",
            params![stream, n + 1],
        )?;
        tx.commit()?;
        Ok(n as u64)
    }

    /// Jump one outgoing stream forward by `gap` (a peer saw higher seqs
    /// from us than we remember).
    pub fn seq_jump(&self, stream: &str, gap: u64) -> Result<()> {
        let gap = i64::try_from(gap).map_err(|_| anyhow!("gap out of range"))?;
        self.c().execute(
            "INSERT INTO seq_out(stream, next) VALUES (?1, ?2 + 1)
             ON CONFLICT(stream) DO UPDATE SET next = next + ?2",
            params![stream, gap],
        )?;
        Ok(())
    }

    /// Jump every outgoing stream, and the floor for streams not seen yet,
    /// forward by `gap` (after a restore).
    pub fn seq_jump_all(&self, gap: u64) -> Result<()> {
        let gap = i64::try_from(gap).map_err(|_| anyhow!("gap out of range"))?;
        let mut c = self.c();
        let tx = c.transaction()?;
        tx.execute("UPDATE seq_out SET next = next + ?1", [gap])?;
        let floor = meta_i64(&tx, META_SEQ_FLOOR)?.unwrap_or(0) + gap;
        tx.execute(
            "INSERT INTO meta(key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![META_SEQ_FLOOR, floor.to_string().into_bytes()],
        )?;
        tx.commit()?;
        Ok(())
    }

    /// Give a queued item a new seq (outbox row and, for DMs, the message).
    pub fn outbox_set_seq(&self, msg_id: &str, seq: u64) -> Result<()> {
        let seq = i64::try_from(seq).map_err(|_| anyhow!("seq out of range"))?;
        let mut c = self.c();
        let tx = c.transaction()?;
        tx.execute(
            "UPDATE outbox SET seq = ?2 WHERE msg_id = ?1",
            params![msg_id, seq],
        )?;
        tx.execute(
            "UPDATE messages SET seq = ?2 WHERE id = ?1",
            params![msg_id, seq],
        )?;
        tx.commit()?;
        Ok(())
    }

    // ---- outbox ----

    /// Queue a DM (text in `messages`).
    pub fn outbox_add(&self, msg_id: &str, peer_id: &str, now: i64) -> Result<()> {
        self.outbox_add_item(
            msg_id,
            peer_id,
            now,
            &OutboxItem::Dm,
            &EnvelopeMeta::default(),
            "default",
        )
    }

    /// Queue any item. Ids are unique across kinds; a duplicate is ignored.
    pub fn outbox_add_item(
        &self,
        msg_id: &str,
        peer_id: &str,
        now: i64,
        item: &OutboxItem,
        meta: &EnvelopeMeta,
        local_identity: &str,
    ) -> Result<()> {
        let (kind, server_id, payload) = match item {
            OutboxItem::Dm => ("dm", None, None),
            OutboxItem::Server { server_id, payload } => {
                ("server", Some(server_id.as_str()), Some(payload.as_slice()))
            }
        };
        self.c().execute(
            "INSERT OR IGNORE INTO outbox(msg_id, peer_id, created_at, kind, server_id, payload, seq, prev_hash, expires_at, local_identity)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                msg_id,
                peer_id,
                now,
                kind,
                server_id,
                payload,
                meta.seq.map(|v| v as i64),
                meta.prev,
                meta.expires_at,
                local_identity
            ],
        )?;
        Ok(())
    }

    pub fn outbox_for(&self, peer_id: &str) -> Result<Vec<OutboxRow>> {
        let c = self.c();
        let mut st = c.prepare(&format!(
            "SELECT {OUTBOX_COLS} FROM outbox WHERE peer_id = ?1 ORDER BY created_at, rowid"
        ))?;
        let rows = st
            .query_map([peer_id], row_to_outbox)?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }

    pub fn outbox_peers(&self) -> Result<Vec<String>> {
        let c = self.c();
        let mut st = c.prepare("SELECT DISTINCT peer_id FROM outbox")?;
        let rows = st
            .query_map([], |r| r.get(0))?
            .collect::<rusqlite::Result<Vec<String>>>()?;
        Ok(rows)
    }

    pub fn outbox_attempt(&self, msg_id: &str, now: i64) -> Result<()> {
        self.c().execute(
            "UPDATE outbox SET attempts = attempts + 1, last_attempt = ?2 WHERE msg_id = ?1",
            params![msg_id, now],
        )?;
        Ok(())
    }

    pub fn outbox_remove(&self, msg_id: &str) -> Result<()> {
        self.c()
            .execute("DELETE FROM outbox WHERE msg_id = ?1", [msg_id])?;
        Ok(())
    }

    /// Remove and return entries created before `cutoff` (all kinds).
    pub fn outbox_expire(&self, cutoff: i64) -> Result<Vec<OutboxRow>> {
        let mut c = self.c();
        let tx = c.transaction()?;
        let rows = {
            let mut st = tx.prepare(&format!(
                "SELECT {OUTBOX_COLS} FROM outbox WHERE created_at < ?1"
            ))?;
            let rows = st
                .query_map([cutoff], row_to_outbox)?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            rows
        };
        tx.execute("DELETE FROM outbox WHERE created_at < ?1", [cutoff])?;
        tx.commit()?;
        Ok(rows)
    }
}

fn meta_i64(c: &Connection, key: &str) -> Result<Option<i64>> {
    Ok(
        c.query_row("SELECT value FROM meta WHERE key = ?1", [key], |r| {
            r.get::<_, Vec<u8>>(0)
        })
        .optional()?
        .and_then(|v| String::from_utf8(v).ok())
        .and_then(|s| s.parse().ok()),
    )
}

/// v2 columns on `contacts` (key-change tracking).
const V2_CONTACT_COLUMNS: &[(&str, &str)] = &[
    ("pending_identity_key", "TEXT"),
    ("rejected_identity_key", "TEXT"),
    ("key_changed_at", "INTEGER"),
];

/// Columns added after v1, per table. New tables are plain
/// `CREATE TABLE IF NOT EXISTS` in [`SCHEMA`]; only columns on existing
/// tables need listing here.
const ADDED_COLUMNS: &[(&str, &[(&str, &str)])] = &[
    ("contacts", V2_CONTACT_COLUMNS),
    // v3: card v2 signatures.
    (
        "contacts",
        &[("card_sig", "TEXT"), ("pending_card_sig", "TEXT")],
    ),
    // v3: generalized outbox (typed items) + reserved envelope fields.
    (
        "outbox",
        &[
            ("kind", "TEXT NOT NULL DEFAULT 'dm'"),
            ("server_id", "TEXT"),
            ("payload", "BLOB"),
            ("seq", "INTEGER"),
            ("prev_hash", "TEXT"),
            ("expires_at", "INTEGER"),
            ("local_identity", "TEXT NOT NULL DEFAULT 'default'"),
        ],
    ),
    (
        "messages",
        &[
            ("seq", "INTEGER"),
            ("prev_hash", "TEXT"),
            ("expires_at", "INTEGER"),
        ],
    ),
];

/// Bring the schema to [`SCHEMA_VERSION`] in one transaction: the column
/// additions and the `schema_version` bump commit together or not at all.
/// Columns are added only if missing (`PRAGMA table_info`), so a database
/// left half-migrated by an older build, or migrated twice, still opens.
/// The column check runs on every open (a few pragma reads), so it does
/// not depend on the recorded version being accurate.
fn migrate(conn: &mut Connection) -> Result<()> {
    let tx = conn.transaction()?;
    // New tables (all `IF NOT EXISTS`) are created inside the same
    // transaction, so a v5 migration that fails leaves no v5 tables behind.
    tx.execute_batch(SCHEMA)?;
    let v: i64 = tx
        .query_row(
            "SELECT value FROM meta WHERE key = 'schema_version'",
            [],
            |r| r.get::<_, Vec<u8>>(0),
        )
        .optional()?
        .and_then(|v| String::from_utf8(v).ok())
        .and_then(|s| s.parse().ok())
        .unwrap_or(1);
    if v > SCHEMA_VERSION {
        bail!("database schema {v} is newer than this build");
    }
    for (table, cols) in ADDED_COLUMNS {
        let have: std::collections::HashSet<String> = {
            let mut st = tx.prepare("SELECT name FROM pragma_table_info(?1)")?;
            let rows = st.query_map([table], |r| r.get::<_, String>(0))?;
            rows.collect::<rusqlite::Result<_>>()?
        };
        for (col, ty) in *cols {
            if !have.contains(*col) {
                tx.execute_batch(&format!("ALTER TABLE {table} ADD COLUMN {col} {ty};"))?;
            }
        }
    }
    if v < SCHEMA_VERSION {
        tx.execute(
            "INSERT INTO meta(key, value) VALUES ('schema_version', ?1)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            [SCHEMA_VERSION.to_string().into_bytes()],
        )?;
    }
    tx.commit()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const KEY: [u8; 32] = [7u8; 32];

    fn msg(id: &str, peer: &str, ts: i64, outgoing: bool) -> Message {
        Message {
            id: id.into(),
            peer_id: peer.into(),
            outgoing,
            body: format!("body {id}"),
            timestamp_ms: ts,
            status: if outgoing {
                DeliveryStatus::Sent
            } else {
                DeliveryStatus::Received
            },
            read: outgoing,
        }
    }

    fn contact(peer: &str) -> ContactRow {
        ContactRow {
            peer_id: peer.into(),
            display_name: String::new(),
            identity_key: "ik".into(),
            ed25519_key: "ed".into(),
            addrs: vec!["/ip4/127.0.0.1/tcp/1".into()],
            verified: false,
            added_at: 1,
            pending_identity_key: None,
            rejected_identity_key: None,
            card_sig: None,
            pending_card_sig: None,
        }
    }

    #[test]
    fn wrong_key_is_rejected_and_file_is_encrypted() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("t.db");
        {
            let s = Store::open(&path, &KEY).unwrap();
            s.meta_set("hello", b"secret-plaintext-marker").unwrap();
        }
        assert!(Store::open(&path, &[8u8; 32]).is_err());
        let s = Store::open(&path, &KEY).unwrap();
        assert_eq!(
            s.meta_get("hello").unwrap().unwrap(),
            b"secret-plaintext-marker"
        );
        drop(s);
        let raw = std::fs::read(&path).unwrap();
        assert!(
            !raw.windows(8).any(|w| w == b"SQLite f"),
            "header must not be plaintext"
        );
        assert!(!raw.windows(13).any(|w| w == b"secret-plaint"));
        assert!(Store::open(&path, &[1u8; 16]).is_err());
    }

    #[test]
    fn migrates_v1_and_tracks_key_changes() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("v1.db");
        {
            // Simulate a v1 database.
            let conn = Connection::open(&path).unwrap();
            conn.execute_batch(&format!("PRAGMA key = \"x'{}'\";", hex::encode(KEY)))
                .unwrap();
            conn.execute_batch(SCHEMA).unwrap();
            conn.execute(
                "INSERT INTO meta(key, value) VALUES ('schema_version', X'31')",
                [],
            )
            .unwrap();
            conn.execute(
                "INSERT INTO contacts(peer_id, identity_key, ed25519_key, verified, added_at) VALUES ('p', 'ik1', 'ed', 1, 1)",
                [],
            )
            .unwrap();
        }
        let s = Store::open(&path, &KEY).unwrap();
        assert_eq!(
            s.meta_get_i64("schema_version").unwrap(),
            Some(SCHEMA_VERSION)
        );
        let c = s.get_contact("p").unwrap().unwrap();
        assert!(c.verified && c.pending_identity_key.is_none());
        assert!(s.accept_pending_key("p").is_err());
        s.set_pending_key("p", "ik2", None, 5).unwrap();
        s.reject_pending_key("p").unwrap();
        let c = s.get_contact("p").unwrap().unwrap();
        assert_eq!(
            (c.identity_key.as_str(), c.rejected_identity_key.as_deref()),
            ("ik1", Some("ik2"))
        );
        s.set_pending_key("p", "ik3", None, 6).unwrap();
        s.accept_pending_key("p").unwrap();
        let c = s.get_contact("p").unwrap().unwrap();
        assert_eq!(c.identity_key, "ik3");
        assert!(
            !c.verified && c.pending_identity_key.is_none() && c.rejected_identity_key.is_none()
        );
        drop(s);
        // Re-open is idempotent.
        Store::open(&path, &KEY).unwrap();
    }

    #[test]
    fn migration_survives_partial_and_repeated_runs() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("partial.db");
        let raw = || {
            let conn = Connection::open(&path).unwrap();
            conn.execute_batch(&format!("PRAGMA key = \"x'{}'\";", hex::encode(KEY)))
                .unwrap();
            conn
        };
        {
            // An older, non-transactional build crashed after the first ALTER:
            // one v2 column exists, schema_version still says 1.
            let conn = raw();
            conn.execute_batch(SCHEMA).unwrap();
            conn.execute_batch(
                "INSERT INTO meta(key, value) VALUES ('schema_version', X'31');
                 ALTER TABLE contacts ADD COLUMN pending_identity_key TEXT;
                 INSERT INTO contacts(peer_id, identity_key, ed25519_key, verified, added_at)
                     VALUES ('p', 'ik1', 'ed', 1, 1);",
            )
            .unwrap();
        }
        let cols = |s: &Store| -> Vec<String> {
            let c = s.conn_for_test();
            let mut st = c
                .prepare("SELECT name FROM pragma_table_info('contacts')")
                .unwrap();
            let v = st
                .query_map([], |r| r.get::<_, String>(0))
                .unwrap()
                .map(Result::unwrap)
                .collect();
            v
        };
        let s = Store::open(&path, &KEY).expect("half-migrated db opens");
        for (table, col) in [
            ("outbox", "kind"),
            ("outbox", "local_identity"),
            ("messages", "expires_at"),
            ("contacts", "card_sig"),
        ] {
            let n: i64 = s
                .conn_for_test()
                .query_row(
                    "SELECT count(*) FROM pragma_table_info(?1) WHERE name = ?2",
                    [table, col],
                    |r| r.get(0),
                )
                .unwrap();
            assert_eq!(n, 1, "{table}.{col}");
        }
        assert_eq!(
            s.meta_get_i64("schema_version").unwrap(),
            Some(SCHEMA_VERSION)
        );
        let after = cols(&s);
        for (c, _) in V2_CONTACT_COLUMNS {
            assert_eq!(after.iter().filter(|x| x == c).count(), 1, "{c}");
        }
        s.set_pending_key("p", "ik2", None, 5).unwrap();
        drop(s);

        // Version marker lost but all columns present: runs again, no error.
        raw()
            .execute_batch("UPDATE meta SET value = X'31' WHERE key = 'schema_version'")
            .unwrap();
        let s = Store::open(&path, &KEY).expect("repeated migration opens");
        assert_eq!(cols(&s), after);
        assert_eq!(
            s.meta_get_i64("schema_version").unwrap(),
            Some(SCHEMA_VERSION)
        );
        let c = s.get_contact("p").unwrap().unwrap();
        assert_eq!(c.pending_identity_key.as_deref(), Some("ik2"));
        drop(s);
    }

    #[test]
    fn failed_migration_rolls_back_completely() {
        let mut conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(SCHEMA).unwrap();
        // Columns get added, then the version bump fails (simulated crash).
        conn.execute_batch(
            "CREATE TRIGGER fail_bump BEFORE INSERT ON meta
             BEGIN SELECT RAISE(ABORT, 'simulated crash'); END;",
        )
        .unwrap();
        assert!(migrate(&mut conn).is_err());
        let n: i64 = conn
            .query_row(
                "SELECT count(*) FROM pragma_table_info('contacts') WHERE name IN
                 ('pending_identity_key', 'rejected_identity_key', 'key_changed_at')",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(n, 0, "no column left behind");
        // Next launch migrates cleanly.
        conn.execute_batch("DROP TRIGGER fail_bump;").unwrap();
        migrate(&mut conn).unwrap();
        migrate(&mut conn).unwrap();
    }

    #[test]
    fn outbox_rows_are_typed_and_scoped() {
        let s = Store::open_in_memory(&KEY).unwrap();
        s.upsert_contact(&contact("p1")).unwrap();
        s.outbox_add("dm1", "p1", 1).unwrap();
        let meta = EnvelopeMeta {
            seq: Some(9),
            prev: Some("h".into()),
            expires_at: Some(10),
        };
        let item = OutboxItem::Server {
            server_id: "srv".into(),
            payload: b"{}".to_vec(),
        };
        s.outbox_add_item("sv1", "p1", 2, &item, &meta, "default")
            .unwrap();
        let rows = s.outbox_for("p1").unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(
            (rows[0].item.clone(), rows[0].meta.clone()),
            (OutboxItem::Dm, EnvelopeMeta::default())
        );
        assert_eq!((rows[1].item.clone(), rows[1].meta.clone()), (item, meta));
        // Removing the contact drops DM rows only.
        s.remove_contact("p1", true).unwrap();
        let rows = s.outbox_for("p1").unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].msg_id, "sv1");
        // Expiry covers every kind.
        assert_eq!(s.outbox_expire(3).unwrap().len(), 1);
        assert!(s.outbox_for("p1").unwrap().is_empty());
    }

    #[test]
    fn contacts_messages_outbox() {
        let s = Store::open_in_memory(&KEY).unwrap();
        s.upsert_contact(&contact("p1")).unwrap();
        s.upsert_contact(&contact("p2")).unwrap();
        s.set_verified("p1", true).unwrap();
        assert!(s.get_contact("p1").unwrap().unwrap().verified);
        assert!(s.set_verified("nope", true).is_err());

        assert!(s.insert_message(&msg("a", "p1", 10, false)).unwrap());
        assert!(
            !s.insert_message(&msg("a", "p1", 10, false)).unwrap(),
            "dedupe"
        );
        s.insert_message(&msg("b", "p1", 20, true)).unwrap();
        s.insert_message(&msg("c", "p1", 30, false)).unwrap();
        let page = s.list_messages("p1", None, 2).unwrap();
        assert_eq!(
            page.iter().map(|m| m.id.as_str()).collect::<Vec<_>>(),
            ["c", "b"]
        );
        let page = s.list_messages("p1", Some(20), 10).unwrap();
        assert_eq!(page.len(), 1);

        let conv = s.list_conversations().unwrap();
        assert_eq!(conv[0].peer_id, "p1");
        assert_eq!(conv[0].unread_count, 2);
        s.mark_read("p1", "a").unwrap();
        assert_eq!(s.list_conversations().unwrap()[0].unread_count, 1);

        s.outbox_add("b", "p1", 100).unwrap();
        assert_eq!(s.outbox_peers().unwrap(), ["p1"]);
        s.outbox_attempt("b", 101).unwrap();
        assert_eq!(s.outbox_for("p1").unwrap()[0].attempts, 1);
        assert_eq!(s.outbox_expire(100).unwrap().len(), 0);
        assert_eq!(s.outbox_expire(101).unwrap().len(), 1);

        s.save_session("p1", "s1", "pickle", 1).unwrap();
        s.save_session("p1", "s2", "pickle2", 2).unwrap();
        assert_eq!(s.sessions_for("p1").unwrap()[0].session_id, "s2");
        s.prune_sessions("p1", 1).unwrap();
        assert_eq!(s.sessions_for("p1").unwrap().len(), 1);

        s.remove_contact("p1", false).unwrap();
        assert!(s.get_contact("p1").unwrap().is_none());
        assert!(s.list_messages("p1", None, 10).unwrap().is_empty());
        assert!(s.sessions_for("p1").unwrap().is_empty());
    }

    #[test]
    fn seq_counters_floor_and_window() {
        let dir = tempfile::tempdir().unwrap();
        let s = Store::open(&dir.path().join("db"), &KEY).unwrap();
        assert_eq!(s.seq_next("dm:a").unwrap(), 1);
        assert_eq!(s.seq_next("dm:a").unwrap(), 2);
        s.seq_jump("dm:a", 100).unwrap();
        assert_eq!(s.seq_next("dm:a").unwrap(), 103);
        s.seq_jump_all(1000).unwrap();
        assert_eq!(s.seq_next("dm:a").unwrap(), 1104);
        // New streams start above the floor too.
        assert_eq!(s.seq_next("dm:b").unwrap(), 1001);
        // Only the most recent SEQ_SEEN_KEEP are kept; the highest stays.
        for i in 1..=(SEQ_SEEN_KEEP as u64 + 10) {
            assert_eq!(s.seq_check("dm", "p", i, b"d").unwrap(), SeqVerdict::Fresh);
            s.seq_commit("dm", "p", i, b"d").unwrap();
        }
        assert_eq!(s.seq_check("dm", "p", 1, b"d").unwrap(), SeqVerdict::Stale);
        assert_eq!(
            s.seq_check("dm", "p", SEQ_SEEN_KEEP as u64 + 10, b"d")
                .unwrap(),
            SeqVerdict::Duplicate
        );
    }

    #[test]
    fn v5_server_tables_migrate_transactionally_and_idempotently() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("v4.db");
        let tables = |s: &Store| -> Vec<String> {
            let c = s.conn_for_test();
            let mut st = c
                .prepare("SELECT name FROM sqlite_master WHERE type = 'table' ORDER BY name")
                .unwrap();
            let v = st
                .query_map([], |r| r.get::<_, String>(0))
                .unwrap()
                .map(Result::unwrap)
                .collect();
            v
        };
        {
            // A v4 database: drop the v5 tables and roll the version back.
            let s = Store::open(&path, &KEY).unwrap();
            s.conn_for_test()
                .execute_batch(
                    "DROP TABLE servers; DROP TABLE manifests; DROP TABLE manifest_forks;
                     DROP TABLE roster_ops; DROP TABLE server_channels; DROP TABLE server_roles;
                     DROP TABLE server_invites; DROP TABLE join_requests; DROP TABLE pending_joins;
                     UPDATE meta SET value = X'34' WHERE key = 'schema_version';",
                )
                .unwrap();
            assert!(!tables(&s).contains(&"servers".to_string()));
        }
        for _ in 0..2 {
            let s = Store::open(&path, &KEY).unwrap();
            let t = tables(&s);
            for want in [
                "servers",
                "manifests",
                "roster_ops",
                "server_channels",
                "server_invites",
                "pending_joins",
            ] {
                assert!(t.contains(&want.to_string()), "{want}");
            }
            assert_eq!(
                s.meta_get_i64("schema_version").unwrap(),
                Some(SCHEMA_VERSION)
            );
        }
        // A newer schema refuses to open and creates nothing.
        let s = Store::open(&path, &KEY).unwrap();
        s.conn_for_test()
            .execute_batch("DROP TABLE pending_joins; UPDATE meta SET value = X'39' WHERE key = 'schema_version';")
            .unwrap();
        drop(s);
        assert!(Store::open(&path, &KEY).is_err());
        let conn = Connection::open(&path).unwrap();
        conn.execute_batch(&format!("PRAGMA key = \"x'{}'\";", hex::encode(KEY)))
            .unwrap();
        let n: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE name = 'pending_joins'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(n, 0, "the failed migration rolled back its CREATE TABLEs");
    }

    #[test]
    fn invites_are_used_up_atomically() {
        use crate::engine::server_model::SignedInvite;
        let dir = tempfile::tempdir().unwrap();
        let s = Store::open(&dir.path().join("db"), &KEY).unwrap();
        s.invite_put(&InviteRow {
            invite_id: "i".into(),
            server_id: "srv".into(),
            secret: vec![1; 32],
            signed: SignedInvite {
                bytes: vec![1],
                sig: vec![2],
            },
            created_at: 0,
            expires_at: 100,
            max_uses: 2,
            uses: 0,
            needs_approval: false,
            revoked: false,
        })
        .unwrap();
        assert!(!s.invite_take_use("i", 100).unwrap(), "expired");
        assert!(s.invite_take_use("i", 50).unwrap());
        assert!(s.invite_take_use("i", 50).unwrap());
        assert!(!s.invite_take_use("i", 50).unwrap(), "used up");
        assert_eq!(s.invite_get("i").unwrap().unwrap().uses, 2);
    }
}
