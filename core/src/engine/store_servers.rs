//! Store: servers Phase 1 tables (schema v5). See the `SCHEMA` comments.

use anyhow::Result;
use rusqlite::{params, OptionalExtension};

use super::Store;
use crate::api::types::ContactCard;
use crate::engine::server_model::{
    cbor, uncbor, Channel, Role, SignedInvite, SignedManifest, SignedOp,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServerRow {
    pub server_id: String,
    pub owner_key: Option<Vec<u8>>,
    pub anchor_key: String,
    pub frozen_seq: Option<i64>,
    pub removed: Option<String>,
    pub created_at: i64,
    /// Per-server display name for our (default) identity.
    pub display_name: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InviteRow {
    pub invite_id: String,
    pub server_id: String,
    pub secret: Vec<u8>,
    pub signed: SignedInvite,
    pub created_at: i64,
    pub expires_at: i64,
    pub max_uses: u32,
    pub uses: u32,
    pub needs_approval: bool,
    pub revoked: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JoinRequestRow {
    pub server_id: String,
    pub peer_id: String,
    pub invite_id: String,
    pub card: ContactCard,
    pub ts: i64,
}

pub fn role_str(r: Role) -> &'static str {
    match r {
        Role::Owner => "owner",
        Role::Admin => "admin",
        Role::Member => "member",
    }
}

pub fn role_parse(s: &str) -> Role {
    match s {
        "owner" => Role::Owner,
        "admin" => Role::Admin,
        _ => Role::Member,
    }
}

fn server_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<ServerRow> {
    Ok(ServerRow {
        server_id: r.get(0)?,
        owner_key: r.get(1)?,
        anchor_key: r.get(2)?,
        frozen_seq: r.get(3)?,
        removed: r.get(4)?,
        created_at: r.get(5)?,
        display_name: r.get(6)?,
    })
}

const SERVER_COLS: &str =
    "server_id, owner_key, anchor_key, frozen_seq, removed, created_at, display_name";

impl Store {
    // ---- servers ----

    pub fn server_insert(&self, s: &ServerRow) -> Result<()> {
        self.c().execute(
            "INSERT INTO servers(server_id, owner_key, anchor_key, frozen_seq, removed, created_at,
                                 display_name)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
             ON CONFLICT(server_id) DO UPDATE SET
               owner_key = COALESCE(excluded.owner_key, servers.owner_key),
               removed = excluded.removed,
               display_name = excluded.display_name",
            params![
                s.server_id,
                s.owner_key,
                s.anchor_key,
                s.frozen_seq,
                s.removed,
                s.created_at,
                s.display_name
            ],
        )?;
        Ok(())
    }

    pub fn server_get(&self, server_id: &str) -> Result<Option<ServerRow>> {
        Ok(self
            .c()
            .query_row(
                &format!("SELECT {SERVER_COLS} FROM servers WHERE server_id = ?1"),
                [server_id],
                server_row,
            )
            .optional()?)
    }

    pub fn servers(&self) -> Result<Vec<ServerRow>> {
        let c = self.c();
        let mut st = c.prepare(&format!(
            "SELECT {SERVER_COLS} FROM servers ORDER BY created_at"
        ))?;
        let rows = st
            .query_map([], server_row)?
            .collect::<rusqlite::Result<_>>()?;
        Ok(rows)
    }

    pub fn server_set_frozen(&self, server_id: &str, seq: i64) -> Result<()> {
        self.c().execute(
            "UPDATE servers SET frozen_seq = ?2 WHERE server_id = ?1",
            params![server_id, seq],
        )?;
        Ok(())
    }

    pub fn server_set_removed(&self, server_id: &str, removed: Option<&str>) -> Result<()> {
        self.c().execute(
            "UPDATE servers SET removed = ?2 WHERE server_id = ?1",
            params![server_id, removed],
        )?;
        Ok(())
    }

    pub fn server_set_display_name(&self, server_id: &str, name: Option<&str>) -> Result<()> {
        self.c().execute(
            "UPDATE servers SET display_name = ?2 WHERE server_id = ?1",
            params![server_id, name],
        )?;
        Ok(())
    }

    pub fn server_set_owner_key(&self, server_id: &str, key: Option<&[u8]>) -> Result<()> {
        self.c().execute(
            "UPDATE servers SET owner_key = ?2 WHERE server_id = ?1",
            params![server_id, key],
        )?;
        Ok(())
    }

    /// Delete everything we keep about a server (leave).
    pub fn server_delete(&self, server_id: &str) -> Result<()> {
        let mut c = self.c();
        let tx = c.transaction()?;
        for t in [
            "servers",
            "manifests",
            "manifest_forks",
            "roster_ops",
            "server_channels",
            "server_roles",
            "server_invites",
            "join_requests",
            "pending_joins",
            "server_memberships",
            "co_members",
        ] {
            tx.execute(
                &format!("DELETE FROM {t} WHERE server_id = ?1"),
                [server_id],
            )?;
        }
        tx.execute(
            "DELETE FROM outbox WHERE server_id = ?1 AND kind = 'server'",
            [server_id],
        )?;
        tx.commit()?;
        Ok(())
    }

    // ---- manifests and ops ----

    pub fn manifest_put(
        &self,
        server_id: &str,
        seq: u64,
        m: &SignedManifest,
        now: i64,
    ) -> Result<()> {
        self.c().execute(
            "INSERT OR REPLACE INTO manifests(server_id, seq, hash, bytes, sig, received_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![server_id, seq as i64, m.hash(), m.bytes, m.sig, now],
        )?;
        Ok(())
    }

    pub fn manifest_latest(&self, server_id: &str) -> Result<Option<SignedManifest>> {
        Ok(self
            .c()
            .query_row(
                "SELECT bytes, sig FROM manifests WHERE server_id = ?1 ORDER BY seq DESC LIMIT 1",
                [server_id],
                |r| {
                    Ok(SignedManifest {
                        bytes: r.get(0)?,
                        sig: r.get(1)?,
                    })
                },
            )
            .optional()?)
    }

    pub fn manifest_at(&self, server_id: &str, seq: u64) -> Result<Option<SignedManifest>> {
        Ok(self
            .c()
            .query_row(
                "SELECT bytes, sig FROM manifests WHERE server_id = ?1 AND seq = ?2",
                params![server_id, seq as i64],
                |r| {
                    Ok(SignedManifest {
                        bytes: r.get(0)?,
                        sig: r.get(1)?,
                    })
                },
            )
            .optional()?)
    }

    pub fn manifest_fork_put(&self, server_id: &str, seq: u64, m: &SignedManifest) -> Result<()> {
        self.c().execute(
            "INSERT OR IGNORE INTO manifest_forks(server_id, hash, seq, bytes, sig) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![server_id, m.hash(), seq as i64, m.bytes, m.sig],
        )?;
        Ok(())
    }

    pub fn manifest_forks(&self, server_id: &str) -> Result<Vec<SignedManifest>> {
        let c = self.c();
        let mut st =
            c.prepare("SELECT bytes, sig FROM manifest_forks WHERE server_id = ?1 ORDER BY seq")?;
        let rows = st
            .query_map([server_id], |r| {
                Ok(SignedManifest {
                    bytes: r.get(0)?,
                    sig: r.get(1)?,
                })
            })?
            .collect::<rusqlite::Result<_>>()?;
        Ok(rows)
    }

    /// Store an op. `false` if we already had it.
    pub fn op_put(&self, server_id: &str, base_seq: u64, op: &SignedOp, now: i64) -> Result<bool> {
        Ok(self.c().execute(
            "INSERT OR IGNORE INTO roster_ops(server_id, hash, base_seq, bytes, sig, received_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![server_id, op.hash(), base_seq as i64, op.bytes, op.sig, now],
        )? == 1)
    }

    /// Ops on top of manifest `base_seq`, oldest first.
    pub fn ops_on(&self, server_id: &str, base_seq: u64) -> Result<Vec<SignedOp>> {
        let c = self.c();
        let mut st = c.prepare(
            "SELECT bytes, sig FROM roster_ops WHERE server_id = ?1 AND base_seq = ?2 ORDER BY received_at, hash",
        )?;
        let rows = st
            .query_map(params![server_id, base_seq as i64], |r| {
                Ok(SignedOp {
                    bytes: r.get(0)?,
                    sig: r.get(1)?,
                })
            })?
            .collect::<rusqlite::Result<_>>()?;
        Ok(rows)
    }

    /// Drop ops older than manifest `seq` (folded in, or dropped, by the owner).
    pub fn ops_prune_below(&self, server_id: &str, seq: u64) -> Result<()> {
        self.c().execute(
            "DELETE FROM roster_ops WHERE server_id = ?1 AND base_seq < ?2",
            params![server_id, seq as i64],
        )?;
        Ok(())
    }

    // ---- effective state ----

    pub fn server_state_replace(
        &self,
        server_id: &str,
        channels: &[Channel],
        roles: &[(String, Role)],
    ) -> Result<()> {
        let mut c = self.c();
        let tx = c.transaction()?;
        tx.execute(
            "DELETE FROM server_channels WHERE server_id = ?1",
            [server_id],
        )?;
        tx.execute("DELETE FROM server_roles WHERE server_id = ?1", [server_id])?;
        for (i, ch) in channels.iter().enumerate() {
            tx.execute(
                "INSERT INTO server_channels(server_id, channel_id, position, channel) VALUES (?1, ?2, ?3, ?4)",
                params![server_id, ch.id, i as i64, cbor(ch)?],
            )?;
        }
        for (p, r) in roles {
            tx.execute(
                "INSERT INTO server_roles(server_id, peer_id, role) VALUES (?1, ?2, ?3)",
                params![server_id, p, role_str(*r)],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    pub fn server_channels(&self, server_id: &str) -> Result<Vec<Channel>> {
        let c = self.c();
        let mut st = c.prepare(
            "SELECT channel FROM server_channels WHERE server_id = ?1 ORDER BY position",
        )?;
        let blobs: Vec<Vec<u8>> = st
            .query_map([server_id], |r| r.get(0))?
            .collect::<rusqlite::Result<_>>()?;
        blobs.iter().map(|b| uncbor(b)).collect()
    }

    pub fn server_roles(&self, server_id: &str) -> Result<Vec<(String, Role)>> {
        let c = self.c();
        let mut st = c.prepare(
            "SELECT peer_id, role FROM server_roles WHERE server_id = ?1 ORDER BY peer_id",
        )?;
        let rows = st
            .query_map([server_id], |r| {
                Ok((r.get::<_, String>(0)?, role_parse(&r.get::<_, String>(1)?)))
            })?
            .collect::<rusqlite::Result<_>>()?;
        Ok(rows)
    }

    // ---- invites ----

    pub fn invite_put(&self, i: &InviteRow) -> Result<()> {
        self.c().execute(
            "INSERT INTO server_invites(invite_id, server_id, secret, signed, created_at, expires_at, max_uses, uses, needs_approval, revoked)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                i.invite_id,
                i.server_id,
                i.secret,
                cbor(&i.signed)?,
                i.created_at,
                i.expires_at,
                i.max_uses,
                i.uses,
                i.needs_approval,
                i.revoked
            ],
        )?;
        Ok(())
    }

    pub fn invite_get(&self, invite_id: &str) -> Result<Option<InviteRow>> {
        let row = self
            .c()
            .query_row(
                "SELECT invite_id, server_id, secret, signed, created_at, expires_at, max_uses, uses, needs_approval, revoked
                 FROM server_invites WHERE invite_id = ?1",
                [invite_id],
                |r| {
                    Ok((
                        r.get::<_, String>(0)?,
                        r.get::<_, String>(1)?,
                        r.get::<_, Vec<u8>>(2)?,
                        r.get::<_, Vec<u8>>(3)?,
                        r.get::<_, i64>(4)?,
                        r.get::<_, i64>(5)?,
                        r.get::<_, u32>(6)?,
                        r.get::<_, u32>(7)?,
                        r.get::<_, bool>(8)?,
                        r.get::<_, bool>(9)?,
                    ))
                },
            )
            .optional()?;
        row.map(
            |(
                invite_id,
                server_id,
                secret,
                signed,
                created_at,
                expires_at,
                max_uses,
                uses,
                needs_approval,
                revoked,
            )| {
                Ok(InviteRow {
                    invite_id,
                    server_id,
                    secret,
                    signed: uncbor(&signed)?,
                    created_at,
                    expires_at,
                    max_uses,
                    uses,
                    needs_approval,
                    revoked,
                })
            },
        )
        .transpose()
    }

    /// Use up one use of a live invite, atomically. `false` if it is
    /// revoked, expired or used up.
    pub fn invite_take_use(&self, invite_id: &str, now: i64) -> Result<bool> {
        Ok(self.c().execute(
            "UPDATE server_invites SET uses = uses + 1
             WHERE invite_id = ?1 AND revoked = 0 AND expires_at > ?2 AND uses < max_uses",
            params![invite_id, now],
        )? == 1)
    }

    pub fn invite_revoke(&self, invite_id: &str) -> Result<bool> {
        Ok(self.c().execute(
            "UPDATE server_invites SET revoked = 1 WHERE invite_id = ?1",
            [invite_id],
        )? == 1)
    }

    // ---- joins ----

    pub fn join_request_put(&self, j: &JoinRequestRow) -> Result<()> {
        self.c().execute(
            "INSERT OR REPLACE INTO join_requests(server_id, peer_id, invite_id, card, ts) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![j.server_id, j.peer_id, j.invite_id, serde_json::to_string(&j.card)?, j.ts],
        )?;
        Ok(())
    }

    pub fn join_request_take(
        &self,
        server_id: &str,
        peer_id: &str,
    ) -> Result<Option<JoinRequestRow>> {
        let mut c = self.c();
        let tx = c.transaction()?;
        let row = tx
            .query_row(
                "SELECT invite_id, card, ts FROM join_requests WHERE server_id = ?1 AND peer_id = ?2",
                params![server_id, peer_id],
                |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, i64>(2)?)),
            )
            .optional()?;
        tx.execute(
            "DELETE FROM join_requests WHERE server_id = ?1 AND peer_id = ?2",
            params![server_id, peer_id],
        )?;
        tx.commit()?;
        row.map(|(invite_id, card, ts)| {
            Ok(JoinRequestRow {
                server_id: server_id.to_string(),
                peer_id: peer_id.to_string(),
                invite_id,
                card: serde_json::from_str(&card)?,
                ts,
            })
        })
        .transpose()
    }

    pub fn join_requests(&self, server_id: &str) -> Result<Vec<JoinRequestRow>> {
        let c = self.c();
        let mut st = c.prepare(
            "SELECT peer_id, invite_id, card, ts FROM join_requests WHERE server_id = ?1 ORDER BY ts",
        )?;
        let rows: Vec<(String, String, String, i64)> = st
            .query_map([server_id], |r| {
                Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?))
            })?
            .collect::<rusqlite::Result<_>>()?;
        rows.into_iter()
            .map(|(peer_id, invite_id, card, ts)| {
                Ok(JoinRequestRow {
                    server_id: server_id.to_string(),
                    peer_id,
                    invite_id,
                    card: serde_json::from_str(&card)?,
                    ts,
                })
            })
            .collect()
    }

    pub fn pending_join_put(&self, server_id: &str, invite: &SignedInvite, now: i64) -> Result<()> {
        self.c().execute(
            "INSERT OR REPLACE INTO pending_joins(server_id, invite, ts) VALUES (?1, ?2, ?3)",
            params![server_id, cbor(invite)?, now],
        )?;
        Ok(())
    }

    pub fn pending_join(&self, server_id: &str) -> Result<Option<SignedInvite>> {
        let b: Option<Vec<u8>> = self
            .c()
            .query_row(
                "SELECT invite FROM pending_joins WHERE server_id = ?1",
                [server_id],
                |r| r.get(0),
            )
            .optional()?;
        b.map(|b| uncbor(&b)).transpose()
    }

    pub fn pending_join_remove(&self, server_id: &str) -> Result<()> {
        self.c().execute(
            "DELETE FROM pending_joins WHERE server_id = ?1",
            [server_id],
        )?;
        Ok(())
    }
}
