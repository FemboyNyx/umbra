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

const SCHEMA_VERSION: i64 = 2;

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
"#;

pub struct Store {
    conn: Mutex<Connection>,
}

#[derive(Debug, Clone)]
pub struct SessionRow {
    pub session_id: String,
    pub pickle: String,
}

#[derive(Debug, Clone)]
pub struct OutboxRow {
    pub msg_id: String,
    pub peer_id: String,
    pub created_at: i64,
    pub attempts: i64,
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
    })
}

const CONTACT_COLS: &str =
    "peer_id, display_name, identity_key, ed25519_key, addrs, verified, added_at, pending_identity_key, rejected_identity_key";

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
        conn.execute_batch(SCHEMA)?;
        let store = Self {
            conn: Mutex::new(conn),
        };
        let v = store.meta_get_i64("schema_version")?.unwrap_or(1);
        if v > SCHEMA_VERSION {
            bail!("database schema {v} is newer than this build");
        }
        if v < 2 {
            // v2: key-change tracking.
            store.c().execute_batch(
                "ALTER TABLE contacts ADD COLUMN pending_identity_key TEXT;
                 ALTER TABLE contacts ADD COLUMN rejected_identity_key TEXT;
                 ALTER TABLE contacts ADD COLUMN key_changed_at INTEGER;",
            )?;
        }
        store.meta_set("schema_version", SCHEMA_VERSION.to_string().as_bytes())?;
        Ok(store)
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

    fn meta_get_i64(&self, key: &str) -> Result<Option<i64>> {
        Ok(self
            .meta_get(key)?
            .and_then(|v| String::from_utf8(v).ok())
            .and_then(|s| s.parse().ok()))
    }

    // ---- contacts ----

    pub fn upsert_contact(&self, c: &ContactRow) -> Result<()> {
        self.c().execute(
            "INSERT INTO contacts(peer_id, display_name, identity_key, ed25519_key, addrs, verified, added_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
             ON CONFLICT(peer_id) DO UPDATE SET display_name = excluded.display_name,
               addrs = excluded.addrs, verified = excluded.verified",
            params![
                c.peer_id,
                c.display_name,
                c.identity_key,
                c.ed25519_key,
                serde_json::to_string(&c.addrs)?,
                c.verified as i64,
                c.added_at
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

    /// Flag a key change (keeps the trusted key in place).
    pub fn set_pending_key(&self, peer_id: &str, key: &str, now: i64) -> Result<()> {
        self.c().execute(
            "UPDATE contacts SET pending_identity_key = ?2, key_changed_at = ?3 WHERE peer_id = ?1",
            params![peer_id, key, now],
        )?;
        Ok(())
    }

    /// Trust the pending key: it replaces the identity key and the contact
    /// becomes unverified.
    pub fn accept_pending_key(&self, peer_id: &str) -> Result<()> {
        let n = self.c().execute(
            "UPDATE contacts SET identity_key = pending_identity_key, pending_identity_key = NULL,
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
               pending_identity_key = NULL, key_changed_at = NULL
             WHERE peer_id = ?1 AND pending_identity_key IS NOT NULL",
            [peer_id],
        )?;
        if n == 0 {
            bail!("no pending key change for {peer_id}");
        }
        Ok(())
    }

    /// Delete contact plus all sessions, messages and outbox entries.
    pub fn remove_contact(&self, peer_id: &str) -> Result<()> {
        let mut c = self.c();
        let tx = c.transaction()?;
        let n = tx.execute("DELETE FROM contacts WHERE peer_id = ?1", [peer_id])?;
        tx.execute("DELETE FROM olm_sessions WHERE peer_id = ?1", [peer_id])?;
        tx.execute("DELETE FROM messages WHERE peer_id = ?1", [peer_id])?;
        tx.execute("DELETE FROM outbox WHERE peer_id = ?1", [peer_id])?;
        tx.commit()?;
        if n == 0 {
            bail!("unknown contact {peer_id}");
        }
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
        let n = self.c().execute(
            "INSERT OR IGNORE INTO messages(id, peer_id, outgoing, body, ts, status, read) VALUES (?1,?2,?3,?4,?5,?6,?7)",
            params![m.id, m.peer_id, m.outgoing as i64, m.body, m.timestamp_ms, status_to_i(m.status), m.read as i64],
        )?;
        Ok(n == 1)
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

    // ---- outbox ----

    pub fn outbox_add(&self, msg_id: &str, peer_id: &str, now: i64) -> Result<()> {
        self.c().execute(
            "INSERT OR IGNORE INTO outbox(msg_id, peer_id, created_at) VALUES (?1, ?2, ?3)",
            params![msg_id, peer_id, now],
        )?;
        Ok(())
    }

    pub fn outbox_for(&self, peer_id: &str) -> Result<Vec<OutboxRow>> {
        let c = self.c();
        let mut st = c.prepare(
            "SELECT msg_id, peer_id, created_at, attempts FROM outbox WHERE peer_id = ?1 ORDER BY created_at, rowid",
        )?;
        let rows = st
            .query_map([peer_id], |r| {
                Ok(OutboxRow {
                    msg_id: r.get(0)?,
                    peer_id: r.get(1)?,
                    created_at: r.get(2)?,
                    attempts: r.get(3)?,
                })
            })?
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

    /// Remove and return ids of entries created before `cutoff`.
    pub fn outbox_expire(&self, cutoff: i64) -> Result<Vec<(String, String)>> {
        let mut c = self.c();
        let tx = c.transaction()?;
        let rows = {
            let mut st = tx.prepare("SELECT msg_id, peer_id FROM outbox WHERE created_at < ?1")?;
            let rows = st
                .query_map([cutoff], |r| Ok((r.get(0)?, r.get(1)?)))?
                .collect::<rusqlite::Result<Vec<(String, String)>>>()?;
            rows
        };
        tx.execute("DELETE FROM outbox WHERE created_at < ?1", [cutoff])?;
        tx.commit()?;
        Ok(rows)
    }
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
        s.set_pending_key("p", "ik2", 5).unwrap();
        s.reject_pending_key("p").unwrap();
        let c = s.get_contact("p").unwrap().unwrap();
        assert_eq!(
            (c.identity_key.as_str(), c.rejected_identity_key.as_deref()),
            ("ik1", Some("ik2"))
        );
        s.set_pending_key("p", "ik3", 6).unwrap();
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

        s.remove_contact("p1").unwrap();
        assert!(s.get_contact("p1").unwrap().is_none());
        assert!(s.list_messages("p1", None, 10).unwrap().is_empty());
        assert!(s.sessions_for("p1").unwrap().is_empty());
    }
}
