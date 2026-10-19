//! Servers, engine side (docs/servers.md, Phase 1 PR 1): creating servers,
//! text channels, roles, authorized-only invites and joining, kicks and bans.
//!
//! State lives in the schema v5 tables (`store_servers.rs`); the rules live
//! in [`crate::engine::server_model`]. Manifests and roster ops travel as
//! `ServerManifest` / `ServerOp` payloads over Olm to every co-member, via
//! the outbox. The owner device folds every accepted op into the next
//! manifest straight away, so ops only live until the next manifest.

use super::*;
use crate::engine::server_model::{
    self as sm, b64url, build_join_link, check_first, check_genesis, check_join_proof, check_next,
    join_proof, key_fingerprint, parse_join_link, rand_bytes, roster_card, sha256_url, Channel,
    Invite, Manifest, ManifestCheck, OpKind, Permission, Policy, Role, RosterEntry, RosterOp,
    ServerKey, ServerState, SignedInvite, SignedManifest, SignedOp,
};
use crate::engine::store::{InviteRow, JoinRequestRow, ServerRow};

const REMOVED_KICKED: &str = "kicked";
/// How many manifests back the owner still rebases an op.
const OP_REBASE_WINDOW: u64 = 16;
const REMOVED_BANNED: &str = "banned";

pub(crate) fn ffi_role(r: Role) -> ServerRole {
    match r {
        Role::Owner => ServerRole::Owner,
        Role::Admin => ServerRole::Admin,
        Role::Member => ServerRole::Member,
    }
}

pub(crate) fn model_role(r: ServerRole) -> Role {
    match r {
        ServerRole::Owner => Role::Owner,
        ServerRole::Admin => Role::Admin,
        ServerRole::Member => Role::Member,
    }
}

fn removed_reason(s: &str) -> MemberLeftReason {
    match s {
        REMOVED_BANNED => MemberLeftReason::Banned,
        REMOVED_KICKED => MemberLeftReason::Kicked,
        _ => MemberLeftReason::Left,
    }
}

impl Core {
    fn server_guard(&self) -> MutexGuard<'_, ()> {
        self.server_lock.lock().expect("server lock poisoned")
    }

    fn me(&self) -> String {
        self.id.peer_id.to_base58()
    }

    fn emit_srv(&self, server_id: &str, event: ServerEvent) {
        self.events.emit(CoreEvent::Server {
            server_id: server_id.to_string(),
            event,
        });
    }

    fn server_row(&self, server_id: &str) -> Result<ServerRow> {
        self.store
            .server_get(server_id)?
            .ok_or_else(|| anyhow!("not in server {server_id}"))
    }

    fn owner_key(row: &ServerRow) -> Result<Option<ServerKey>> {
        row.owner_key
            .as_deref()
            .map(ServerKey::from_secret)
            .transpose()
    }

    /// Latest manifest with the stored ops on it applied, in a deterministic
    /// order (`ts`, then hash) so every member gets the same result.
    pub(crate) fn server_state(&self, server_id: &str) -> Result<Option<ServerState>> {
        let Some(signed) = self.store.manifest_latest(server_id)? else {
            return Ok(None);
        };
        let mut st = ServerState::new(server_id, signed)?;
        let mut ops: Vec<(String, RosterOp)> = self
            .store
            .ops_on(server_id, st.base.seq)?
            .into_iter()
            .filter_map(|o| o.verified().ok().map(|r| (o.hash(), r)))
            .collect();
        ops.sort_by(|a, b| (a.1.ts, &a.0).cmp(&(b.1.ts, &b.0)));
        for (h, op) in ops {
            let ts = op.ts; // re-applying later must not expire the invite
            let _ = st.apply_op(&h, op, ts);
        }
        Ok(Some(st))
    }

    /// State for a local change: we must be an active member.
    fn active_state(&self, server_id: &str) -> Result<(ServerRow, ServerState)> {
        let row = self.server_row(server_id)?;
        if row.frozen_seq.is_some() {
            bail!("server is frozen (forked manifest); it is read-only");
        }
        if row.removed.is_some() {
            bail!("you are no longer in this server");
        }
        if self.store.pending_join(server_id)?.is_some() {
            bail!("still joining this server");
        }
        let st = self
            .server_state(server_id)?
            .ok_or_else(|| anyhow!("no manifest for {server_id}"))?;
        if st.role_of(&self.me()).is_none() {
            bail!("you are not in this server's roster");
        }
        Ok((row, st))
    }

    /// Our card as shown in one server: the default identity, with the
    /// per-server display name if set (the card signature covers only the
    /// PeerId and identity key, so the name can differ per server).
    fn server_card(&self, display_name: Option<&str>) -> ContactCard {
        let mut c = self.self_card();
        if let Some(n) = display_name {
            c.display_name = Some(n.to_string());
        }
        c
    }

    // ---- public API ----

    /// Create a server we own. `recovery_keys` are 1 to 3 Ed25519 public
    /// keys (base64) that can take the server over if the owner key is lost;
    /// if empty, one is generated and its secret returned once in
    /// [`CreatedServer::recovery_secret`] (never stored). The owner key `K_S`
    /// stays in the encrypted store and never crosses the FFI.
    pub fn create_server(
        self: &Arc<Self>,
        name: &str,
        recovery_keys: Vec<String>,
        display_name: Option<String>,
    ) -> Result<CreatedServer> {
        let display_name = clean_display_name(display_name)?;
        let _g = self.server_guard();
        let key = ServerKey::generate();
        let (recovery, recovery_secret) = if recovery_keys.is_empty() {
            let r = ServerKey::generate();
            (vec![r.public_b64()], Some(b64url(&r.secret())))
        } else {
            (recovery_keys, None)
        };
        let me = self.me();
        let m = Manifest {
            v: 1,
            server_id: None,
            seq: 0,
            prev: None,
            owner: key.public_b64(),
            recovery,
            name: name.trim().to_string(),
            icon: None,
            channels: vec![Channel::new_text("general")],
            roster: vec![RosterEntry {
                card: roster_card(&self.server_card(display_name.as_deref()))?,
                role: Role::Owner,
                added_by: me,
                added_seq: 0,
            }],
            bans: Vec::new(),
            hosts: Vec::new(),
            packs: Vec::new(),
            policy: Policy::default(),
            takeover: false,
        };
        let signed = SignedManifest::sign(&m, &key)?;
        let server_id = check_genesis(&signed)?;
        self.add_server_membership(
            &server_id,
            ServerIdentity::Default,
            Some(HistoryVisibility::Shared),
        )?;
        let now = now_ms();
        self.store.server_insert(&ServerRow {
            server_id: server_id.clone(),
            owner_key: Some(key.secret().to_vec()),
            anchor_key: key.public_b64(),
            frozen_seq: None,
            removed: None,
            created_at: now,
            display_name,
        })?;
        self.store.manifest_put(&server_id, 0, &signed, now)?;
        let st = self
            .server_state(&server_id)?
            .ok_or_else(|| anyhow!("manifest vanished"))?;
        self.settle(None, &st)?;
        Ok(CreatedServer {
            server: self.server_info(&server_id)?,
            recovery_secret,
        })
    }

    pub fn server_info(&self, server_id: &str) -> Result<ServerInfo> {
        let row = self.server_row(server_id)?;
        let joining = self.store.pending_join(server_id)?;
        let st = self.server_state(server_id)?;
        let me = self.me();
        let status = if row.removed.is_some() {
            ServerStatus::Removed
        } else if row.frozen_seq.is_some() {
            ServerStatus::Frozen
        } else if joining.is_some() {
            ServerStatus::Joining
        } else {
            ServerStatus::Active
        };
        let owner_fingerprint = key_fingerprint(&row.anchor_key).unwrap_or_default();
        let Some(st) = st else {
            let name = joining
                .and_then(|j| j.verified().ok())
                .map(|i| i.server_name)
                .unwrap_or_default();
            return Ok(ServerInfo {
                server_id: server_id.to_string(),
                name,
                owner_fingerprint,
                my_role: None,
                status,
                removed_reason: row.removed.as_deref().map(removed_reason),
                seq: 0,
                member_count: 0,
                channel_count: 0,
                invite_roles: Vec::new(),
                history_shared: true,
                retention_days: sm::MAX_RETENTION_DAYS,
                is_owner_device: false,
                my_display_name: row.display_name.clone(),
            });
        };
        let m = &st.manifest;
        Ok(ServerInfo {
            server_id: server_id.to_string(),
            name: m.name.clone(),
            owner_fingerprint: key_fingerprint(&st.base.owner).unwrap_or(owner_fingerprint),
            my_role: if status == ServerStatus::Removed {
                None
            } else {
                st.role_of(&me).map(ffi_role)
            },
            status,
            removed_reason: row.removed.as_deref().map(removed_reason),
            seq: st.base.seq,
            member_count: m.roster.len() as u32,
            channel_count: m.channels.len() as u32,
            invite_roles: m.policy.invite.iter().copied().map(ffi_role).collect(),
            history_shared: m.policy.history == HistoryVisibility::Shared,
            retention_days: m.policy.retention_days,
            is_owner_device: row.owner_key.is_some(),
            my_display_name: row.display_name.clone(),
        })
    }

    pub fn list_servers(&self) -> Result<Vec<ServerInfo>> {
        self.store
            .servers()?
            .iter()
            .map(|r| self.server_info(&r.server_id))
            .collect()
    }

    pub fn list_channels(&self, server_id: &str) -> Result<Vec<ServerChannel>> {
        let st = self
            .server_state(server_id)?
            .ok_or_else(|| anyhow!("no manifest for {server_id} yet"))?;
        let me = self.me();
        Ok(st
            .channels()
            .iter()
            .map(|c| ServerChannel {
                id: c.id.clone(),
                name: c.name.clone(),
                can_read: st.can_read(&me, &c.id),
                can_post: st.can_post(&me, &c.id),
                read_roles: c.read.iter().copied().map(ffi_role).collect(),
                post_roles: c.post.iter().copied().map(ffi_role).collect(),
            })
            .collect())
    }

    pub fn list_members(&self, server_id: &str) -> Result<Vec<ServerMember>> {
        let st = self
            .server_state(server_id)?
            .ok_or_else(|| anyhow!("no manifest for {server_id} yet"))?;
        let me = self.me();
        Ok(st
            .members()
            .iter()
            .map(|e| ServerMember {
                peer_id: e.card.peer_id.clone(),
                display_name: e.card.display_name.clone().unwrap_or_default(),
                role: ffi_role(e.role),
                added_by: e.added_by.clone(),
                is_self: e.card.peer_id == me,
                is_contact: self
                    .store
                    .get_contact(&e.card.peer_id)
                    .ok()
                    .flatten()
                    .is_some(),
            })
            .collect())
    }

    /// New text channel (owner, or admins via a `CreateChannel` op).
    pub fn create_channel(self: &Arc<Self>, server_id: &str, name: &str) -> Result<ServerChannel> {
        let _g = self.server_guard();
        let (row, st) = self.active_state(server_id)?;
        let ch = Channel::new_text(name.trim());
        if let Some(key) = Self::owner_key(&row)? {
            let mut next = st.next_manifest();
            next.channels.push(ch.clone());
            self.owner_commit(&st, next, &key)?;
        } else {
            self.submit_op(
                st,
                OpKind::CreateChannel {
                    channel: ch.clone(),
                },
                None,
            )?;
        }
        drop(_g);
        self.list_channels(server_id)?
            .into_iter()
            .find(|c| c.id == ch.id)
            .ok_or_else(|| anyhow!("channel vanished"))
    }

    /// Rename a channel (owner, or admins via an `EditChannel` op).
    pub fn rename_channel(
        self: &Arc<Self>,
        server_id: &str,
        channel_id: &str,
        name: &str,
    ) -> Result<()> {
        let _g = self.server_guard();
        let (row, st) = self.active_state(server_id)?;
        let mut ch = st
            .channels()
            .iter()
            .find(|c| c.id == channel_id)
            .cloned()
            .ok_or_else(|| anyhow!("no such channel"))?;
        ch.name = name.trim().to_string();
        if let Some(key) = Self::owner_key(&row)? {
            let mut next = st.next_manifest();
            for c in next.channels.iter_mut().filter(|c| c.id == channel_id) {
                *c = ch.clone();
            }
            self.owner_commit(&st, next, &key)
        } else {
            self.submit_op(st, OpKind::EditChannel { channel: ch }, None)
        }
    }

    /// Owner only: make a member an admin or a member again. Ownership
    /// transfer is not part of this API yet.
    pub fn set_role(
        self: &Arc<Self>,
        server_id: &str,
        peer_id: &str,
        role: ServerRole,
    ) -> Result<()> {
        let _g = self.server_guard();
        let (row, st) = self.active_state(server_id)?;
        let key = Self::owner_key(&row)?.ok_or_else(|| anyhow!("only the owner changes roles"))?;
        if role == ServerRole::Owner {
            bail!("ownership transfer is not supported yet");
        }
        if peer_id == self.me() {
            bail!("the owner's role can't change");
        }
        let mut next = st.next_manifest();
        let e = next
            .roster
            .iter_mut()
            .find(|e| e.card.peer_id == peer_id)
            .ok_or_else(|| anyhow!("{peer_id} is not a member"))?;
        e.role = model_role(role);
        self.owner_commit(&st, next, &key)
    }

    /// Owner only: which roles may create invites (owner always may).
    pub fn set_invite_roles(
        self: &Arc<Self>,
        server_id: &str,
        roles: Vec<ServerRole>,
    ) -> Result<()> {
        let _g = self.server_guard();
        let (row, st) = self.active_state(server_id)?;
        let key =
            Self::owner_key(&row)?.ok_or_else(|| anyhow!("only the owner changes permissions"))?;
        let mut next = st.next_manifest();
        let mut r: Vec<Role> = roles.into_iter().map(model_role).collect();
        if !r.contains(&Role::Owner) {
            r.insert(0, Role::Owner);
        }
        r.sort();
        r.dedup();
        next.policy.invite = r;
        self.owner_commit(&st, next, &key)
    }

    /// Owner only: whether new members see history from before they joined
    /// (`policy.history`). Default on (shared).
    pub fn set_history_shared(self: &Arc<Self>, server_id: &str, shared: bool) -> Result<()> {
        let _g = self.server_guard();
        let (row, st) = self.active_state(server_id)?;
        let key =
            Self::owner_key(&row)?.ok_or_else(|| anyhow!("only the owner changes history"))?;
        let mut next = st.next_manifest();
        next.policy.history = if shared {
            HistoryVisibility::Shared
        } else {
            HistoryVisibility::Joined
        };
        self.owner_commit(&st, next, &key)
    }

    /// Our display name in this server (`None` = the global one). Stored at
    /// once and used for invites and joins; on the owner device the roster
    /// entry is updated straight away. Other members' roster entries keep
    /// the name they joined with until the owner rewrites them (no profile
    /// op in the PR 1 protocol).
    pub fn set_server_display_name(
        self: &Arc<Self>,
        server_id: &str,
        display_name: Option<String>,
    ) -> Result<()> {
        let name = clean_display_name(display_name)?;
        let _g = self.server_guard();
        let row = self.server_row(server_id)?;
        self.store
            .server_set_display_name(server_id, name.as_deref())?;
        let Some(key) = Self::owner_key(&row)? else {
            return Ok(());
        };
        let (_, st) = self.active_state(server_id)?;
        let me = self.me();
        let card = roster_card(&self.server_card(name.as_deref()))?;
        let mut next = st.next_manifest();
        for e in next.roster.iter_mut().filter(|e| e.card.peer_id == me) {
            e.card = card.clone();
        }
        self.owner_commit(&st, next, &key)
    }

    pub fn kick(self: &Arc<Self>, server_id: &str, peer_id: &str) -> Result<()> {
        self.moderate(server_id, peer_id, Moderation::Kick)
    }

    pub fn ban(self: &Arc<Self>, server_id: &str, peer_id: &str) -> Result<()> {
        self.moderate(server_id, peer_id, Moderation::Ban)
    }

    pub fn unban(self: &Arc<Self>, server_id: &str, peer_id: &str) -> Result<()> {
        self.moderate(server_id, peer_id, Moderation::Unban)
    }

    fn moderate(self: &Arc<Self>, server_id: &str, peer_id: &str, what: Moderation) -> Result<()> {
        let _g = self.server_guard();
        let (row, st) = self.active_state(server_id)?;
        if peer_id == self.me() {
            bail!("you can't kick or ban yourself; use leave_server");
        }
        if let Some(key) = Self::owner_key(&row)? {
            let mut next = st.next_manifest();
            let seq = next.seq;
            match what {
                Moderation::Kick => {
                    if !next.roster.iter().any(|e| e.card.peer_id == peer_id) {
                        bail!("{peer_id} is not a member");
                    }
                    next.roster.retain(|e| e.card.peer_id != peer_id);
                }
                Moderation::Ban => {
                    next.roster.retain(|e| e.card.peer_id != peer_id);
                    if !next.bans.iter().any(|b| b.peer_id == peer_id) {
                        next.bans.push(sm::Ban {
                            peer_id: peer_id.to_string(),
                            seq,
                        });
                    }
                }
                Moderation::Unban => {
                    if !next.bans.iter().any(|b| b.peer_id == peer_id) {
                        bail!("{peer_id} is not banned");
                    }
                    next.bans.retain(|b| b.peer_id != peer_id);
                }
            }
            self.owner_commit(&st, next, &key)
        } else {
            let p = peer_id.to_string();
            let op = match what {
                Moderation::Kick => OpKind::Kick { peer_id: p },
                Moderation::Ban => OpKind::Ban { peer_id: p },
                Moderation::Unban => OpKind::Unban { peer_id: p },
            };
            self.submit_op(st, op, None)
        }
    }

    /// Leave a server and delete everything we keep about it. The owner
    /// can only leave a server nobody else is in (no ownership transfer
    /// yet). Other members find out when the owner drops us; a signed
    /// `Leave` is not in the PR 1 protocol.
    pub fn leave_server(&self, server_id: &str) -> Result<()> {
        let _g = self.server_guard();
        let row = self.server_row(server_id)?;
        if row.owner_key.is_some() {
            if let Some(st) = self.server_state(server_id)? {
                if st.members().len() > 1 && row.removed.is_none() {
                    bail!("the owner can't leave while others are in the server");
                }
            }
        }
        let peers: Vec<String> = self
            .store
            .list_co_members(Some(server_id))?
            .into_iter()
            .map(|r| r.peer_id)
            .collect();
        self.store.server_delete(server_id)?;
        for p in peers {
            if self.store.co_member_rows(&p)?.is_empty() && self.store.get_contact(&p)?.is_none() {
                self.crypto.forget_sessions(&p)?;
            }
        }
        Ok(())
    }

    /// Create an invite link. Only roles in `policy.invite` may (default
    /// owner and admin): invites are authorized-only.
    pub fn create_invite(&self, server_id: &str, opts: InviteOptions) -> Result<CreatedInvite> {
        let _g = self.server_guard();
        let (_, st) = self.active_state(server_id)?;
        if !st.can(&self.me(), Permission::Invite) {
            bail!("your role may not create invites in this server");
        }
        let ttl_ms = match opts.ttl_secs {
            Some(0) => bail!("invite ttl must be positive"),
            Some(s) => i64::try_from(s.saturating_mul(1000)).unwrap_or(i64::MAX),
            None => sm::DEFAULT_INVITE_TTL_MS,
        };
        let max_uses = opts.max_uses.unwrap_or(sm::DEFAULT_INVITE_USES);
        if max_uses == 0 || max_uses > sm::MAX_INVITE_USES {
            bail!("max_uses must be 1 to {}", sm::MAX_INVITE_USES);
        }
        let secret = rand_bytes::<32>();
        let invite_id = b64url(&rand_bytes::<16>());
        let now = now_ms();
        let expires_at = now.saturating_add(ttl_ms);
        let inv = Invite {
            server_id: server_id.to_string(),
            owner_key: st.base.owner.clone(),
            server_name: st.manifest.name.clone(),
            inviter: self.self_card(),
            invite_id: invite_id.clone(),
            secret_hash: sha256_url(&secret),
            expires_at,
            max_uses,
            needs_approval: opts.needs_approval,
            hosts: st.manifest.hosts.clone(),
        };
        let signed = SignedInvite::sign(&inv, &self.id.keypair)?;
        self.store.invite_put(&InviteRow {
            invite_id: invite_id.clone(),
            server_id: server_id.to_string(),
            secret: secret.to_vec(),
            signed: signed.clone(),
            created_at: now,
            expires_at,
            max_uses,
            uses: 0,
            needs_approval: opts.needs_approval,
            revoked: false,
        })?;
        Ok(CreatedInvite {
            invite_id,
            link: build_join_link(&signed, &secret)?,
            expires_at_ms: expires_at,
            max_uses,
        })
    }

    /// Revoke one of our invites (no further joins with it).
    pub fn revoke_invite(&self, invite_id: &str) -> Result<()> {
        if !self.store.invite_revoke(invite_id)? {
            bail!("no such invite");
        }
        Ok(())
    }

    /// Check an invite link and describe it, without joining.
    pub fn parse_invite(&self, link: &str) -> Result<InvitePreview> {
        let (_, inv, _) = parse_join_link(link)?;
        let already_member = self
            .store
            .server_get(&inv.server_id)?
            .is_some_and(|r| r.removed.is_none());
        Ok(InvitePreview {
            owner_fingerprint: key_fingerprint(&inv.owner_key)?,
            expired: inv.expires_at <= now_ms(),
            hosts: inv.hosts.iter().map(|h| h.peer_id.clone()).collect(),
            server_id: inv.server_id,
            server_name: inv.server_name,
            inviter: inv.inviter,
            invite_id: inv.invite_id,
            expires_at_ms: inv.expires_at,
            needs_approval: inv.needs_approval,
            already_member,
        })
    }

    /// Join with an invite link: sends `JoinHello` to the inviter (needs the
    /// network). On `Ok` the server shows as `Joining` until the first
    /// manifest arrives (`ServerEvent::Joined`); approval invites wait for
    /// the inviter. A refusal undoes everything and returns the reason.
    pub async fn join_server(
        self: &Arc<Self>,
        link: &str,
        display_name: Option<String>,
    ) -> Result<ServerInfo> {
        let net = self.net_handle()?;
        let (server_id, peer, addrs, req) = self.prepare_join(link, display_name)?;
        net.add_addrs(peer, addrs);
        let resp = net.msg(peer, req).await;
        self.finish_join(&server_id, resp)
    }

    /// Local half of [`Self::join_server`]: record the pending join and the
    /// inviter as a co-member, and build the `JoinHello`.
    pub(crate) fn prepare_join(
        self: &Arc<Self>,
        link: &str,
        display_name: Option<String>,
    ) -> Result<(String, PeerId, Vec<Multiaddr>, MsgRequest)> {
        let display_name = clean_display_name(display_name)?;
        let (signed, inv, secret) = parse_join_link(link)?;
        if inv.expires_at <= now_ms() {
            bail!("this invite has expired");
        }
        if inv.inviter.peer_id == self.me() {
            bail!("this is your own invite");
        }
        let server_id = inv.server_id.clone();
        let _g = self.server_guard();
        if let Some(r) = self.store.server_get(&server_id)? {
            if r.removed.is_none() && self.store.pending_join(&server_id)?.is_none() {
                bail!("already in this server");
            }
            self.store.server_delete(&server_id)?; // rejoin after removal / retry
        }
        self.add_server_membership(&server_id, ServerIdentity::Default, None)?;
        let now = now_ms();
        self.store.server_insert(&ServerRow {
            server_id: server_id.clone(),
            owner_key: None,
            anchor_key: inv.owner_key.clone(),
            frozen_seq: None,
            removed: None,
            created_at: now,
            display_name: display_name.clone(),
        })?;
        self.store.pending_join_put(&server_id, &signed, now)?;
        if let Err(e) = self.add_co_member(&server_id, inv.inviter.clone()) {
            self.store.server_delete(&server_id)?;
            return Err(e);
        }
        let peer = parse_peer(&inv.inviter.peer_id)?;
        let req = MsgRequest::JoinHello {
            card: self.server_card(display_name.as_deref()),
            server_id: server_id.clone(),
            invite_id: inv.invite_id.clone(),
            proof: join_proof(&secret, &self.id.peer_id),
        };
        Ok((server_id, peer, parse_addrs(&inv.inviter.addrs), req))
    }

    /// The inviter's answer to our `JoinHello`; a refusal undoes the join.
    pub(crate) fn finish_join(
        &self,
        server_id: &str,
        resp: Result<MsgResponse>,
    ) -> Result<ServerInfo> {
        let err = match resp {
            Ok(MsgResponse::Ok) => return self.server_info(server_id),
            Ok(MsgResponse::Rejected(r)) => anyhow!("join refused: {r:?}"),
            Ok(other) => anyhow!("unexpected join response {other:?}"),
            Err(e) => e.context("could not reach the inviter"),
        };
        let _g = self.server_guard();
        if self.store.pending_join(server_id)?.is_some() {
            self.store.server_delete(server_id)?;
        }
        Err(err)
    }

    /// Pending requests for our approval-required invites.
    pub fn join_requests(&self, server_id: &str) -> Result<Vec<JoinRequest>> {
        Ok(self
            .store
            .join_requests(server_id)?
            .into_iter()
            .map(join_request)
            .collect())
    }

    /// Admit (or drop) a pending join request. Admitting uses one invite use.
    pub fn approve_join(
        self: &Arc<Self>,
        server_id: &str,
        peer_id: &str,
        approve: bool,
    ) -> Result<()> {
        let _g = self.server_guard();
        let req = self
            .store
            .join_request_take(server_id, peer_id)?
            .ok_or_else(|| anyhow!("no join request from {peer_id}"))?;
        if !approve {
            return Ok(());
        }
        let (_, st) = self.active_state(server_id)?;
        if st.is_banned(peer_id) {
            bail!("{peer_id} is banned");
        }
        if !st.can(&self.me(), Permission::Invite) {
            bail!("your role may no longer admit members");
        }
        if !self.store.invite_take_use(&req.invite_id, now_ms())? {
            bail!("that invite expired, was revoked or is used up");
        }
        let inv = self
            .store
            .invite_get(&req.invite_id)?
            .ok_or_else(|| anyhow!("invite vanished"))?;
        self.admit(st, req.card, inv.signed)
    }

    // ---- inbound ----

    /// `JoinHello` from a would-be member (any peer; rate-limited).
    pub(super) fn on_join_hello(
        self: &Arc<Self>,
        peer: PeerId,
        card: ContactCard,
        server_id: &str,
        invite_id: &str,
        proof: &[u8],
    ) -> MsgResponse {
        if !self.lim().join_hello.try_take(&peer, Instant::now()) {
            return MsgResponse::Rejected(RejectReason::RateLimited);
        }
        match qr::validate_card(&card) {
            Ok(p) if p == peer && card.sig.is_some() => {}
            _ => return MsgResponse::Rejected(RejectReason::Malformed),
        }
        let _g = self.server_guard();
        match self.try_join(peer, card, server_id, invite_id, proof) {
            Ok(r) => r,
            Err(e) => {
                tracing::warn!(%peer, %server_id, "join failed: {e:#}");
                MsgResponse::Rejected(RejectReason::Internal)
            }
        }
    }

    fn try_join(
        self: &Arc<Self>,
        peer: PeerId,
        card: ContactCard,
        server_id: &str,
        invite_id: &str,
        proof: &[u8],
    ) -> Result<MsgResponse> {
        let reject = |r| Ok(MsgResponse::Rejected(r));
        let now = now_ms();
        let Some(inv) = self.store.invite_get(invite_id)? else {
            return reject(RejectReason::InviteInvalid);
        };
        if inv.server_id != server_id
            || inv.revoked
            || inv.expires_at <= now
            || !check_join_proof(&inv.secret, &peer, proof)
        {
            return reject(RejectReason::InviteInvalid);
        }
        let Some(row) = self.store.server_get(server_id)? else {
            return reject(RejectReason::InviteInvalid);
        };
        if row.frozen_seq.is_some() || row.removed.is_some() {
            return reject(RejectReason::NotAuthorized);
        }
        let Some(st) = self.server_state(server_id)? else {
            return reject(RejectReason::NotAuthorized);
        };
        let pid = peer.to_base58();
        if st.is_banned(&pid) {
            return reject(RejectReason::Banned);
        }
        if let Some(h) = self.ns().handle.clone() {
            h.add_addrs(peer, parse_addrs(&card.addrs));
        }
        if st.role_of(&pid).is_some() {
            // Already in (the joiner lost our reply): send the state again.
            self.send_state_to(&pid, &st)?;
            return Ok(MsgResponse::Ok);
        }
        if !st.can(&self.me(), Permission::Invite) {
            return reject(RejectReason::NotAuthorized);
        }
        if st.members().len() >= sm::MAX_MEMBERS {
            return reject(RejectReason::NotAuthorized);
        }
        if inv.uses >= inv.max_uses {
            return reject(RejectReason::InviteInvalid);
        }
        if inv.needs_approval {
            let r = JoinRequestRow {
                server_id: server_id.to_string(),
                peer_id: pid,
                invite_id: invite_id.to_string(),
                card,
                ts: now,
            };
            self.store.join_request_put(&r)?;
            self.emit_srv(
                server_id,
                ServerEvent::JoinRequest {
                    request: join_request(r),
                },
            );
            return Ok(MsgResponse::Ok);
        }
        if !self.store.invite_take_use(invite_id, now)? {
            return reject(RejectReason::InviteInvalid);
        }
        self.admit(st, card, inv.signed)?;
        Ok(MsgResponse::Ok)
    }

    /// `ServerManifest` / `ServerOp` from a co-member (already decrypted and
    /// checked to be from a co-member of `server_id`).
    pub(super) fn on_server_payload(
        self: &Arc<Self>,
        from: &str,
        server_id: &str,
        kind: ServerPayloadKind,
        data: &[u8],
    ) -> Result<()> {
        let _g = self.server_guard();
        match kind {
            ServerPayloadKind::ServerManifest => self.on_manifest(server_id, data),
            ServerPayloadKind::ServerOp => self.on_op(from, server_id, data),
            _ => Ok(()),
        }
    }

    fn on_manifest(self: &Arc<Self>, server_id: &str, data: &[u8]) -> Result<()> {
        let signed = SignedManifest::from_wire(data)?;
        let row = self.server_row(server_id)?;
        let now = now_ms();
        let Some(cur) = self.server_state(server_id)? else {
            // First manifest after join_server: anchored on the invite's owner key.
            check_first(server_id, &row.anchor_key, &signed)?;
            let m = signed.manifest()?;
            self.store.manifest_put(server_id, m.seq, &signed, now)?;
            let st = self
                .server_state(server_id)?
                .ok_or_else(|| anyhow!("manifest vanished"))?;
            return self.settle(None, &st);
        };
        if row.frozen_seq.is_some() {
            return Ok(()); // read-only until the user resolves the fork
        }
        match check_next(server_id, &cur.signed, &signed)? {
            ManifestCheck::Accept => {
                let m = signed.manifest()?;
                self.store.manifest_put(server_id, m.seq, &signed, now)?;
                self.store.ops_prune_below(server_id, m.seq)?;
                if let Some(k) = Self::owner_key(&row)? {
                    if k.public_b64() != m.owner {
                        // Taken over by a recovery key: our K_S is void.
                        self.store.server_set_owner_key(server_id, None)?;
                    }
                }
                let st = self
                    .server_state(server_id)?
                    .ok_or_else(|| anyhow!("manifest vanished"))?;
                self.settle(Some(&cur.manifest), &st)
            }
            ManifestCheck::Duplicate => Ok(()),
            ManifestCheck::Rollback { have, got } => {
                tracing::warn!(%server_id, have, got, "refused a manifest rollback");
                self.emit_srv(server_id, ServerEvent::ManifestRollback { have, got });
                Ok(())
            }
            ManifestCheck::Fork { seq } => {
                tracing::error!(%server_id, seq, "two manifests for one seq: freezing the server");
                self.store.manifest_fork_put(server_id, seq, &signed)?;
                self.store.server_set_frozen(server_id, seq as i64)?;
                self.emit_srv(server_id, ServerEvent::Forked { seq });
                Ok(())
            }
        }
    }

    fn on_op(self: &Arc<Self>, from: &str, server_id: &str, data: &[u8]) -> Result<()> {
        let signed = SignedOp::from_wire(data)?;
        let op = signed.verified()?;
        if op.server_id != server_id {
            bail!("op for another server");
        }
        let row = self.server_row(server_id)?;
        if row.frozen_seq.is_some() || row.removed.is_some() {
            return Ok(());
        }
        let now = now_ms();
        let Some(st) = self.server_state(server_id)? else {
            // Joining: the manifest comes first in the outbox, but keep it.
            self.store.op_put(server_id, op.base_seq, &signed, now)?;
            return Ok(());
        };
        if op.base_seq > st.base.seq {
            self.store.op_put(server_id, op.base_seq, &signed, now)?; // applies once we catch up
            return Ok(());
        }
        let owner = Self::owner_key(&row)?;
        let mut op = op;
        if op.base_seq < st.base.seq {
            // The owner rebases a recent op it hasn't folded yet (e.g. two
            // admin ops on one base). The signer must hold the permission in
            // the current manifest too; unbans are never rebased (they need
            // a newer base than the ban).
            let recent = st.base.seq - op.base_seq <= OP_REBASE_WINDOW;
            if owner.is_none() || !recent || matches!(op.op, OpKind::Unban { .. }) {
                tracing::debug!(%server_id, %from, "dropping an op on an older manifest");
                return Ok(());
            }
            op.base_seq = st.base.seq;
        }
        let mut next = st.clone();
        if let Err(e) = next.apply_op(&signed.hash(), op, now) {
            bail!("op refused: {e:?}");
        }
        if !self.store.op_put(server_id, st.base.seq, &signed, now)? {
            return Ok(()); // seen it
        }
        if let Some(key) = owner {
            // Owner device: fold the op into the next manifest right away.
            let m = next.next_manifest();
            return self.owner_commit_from(&st.manifest, &next, m, &key);
        }
        self.settle(Some(&st.manifest), &next)
    }

    // ---- internals ----

    /// Owner: sign `next` (seq + 1 of `st`), adopt it and send it to every
    /// member (and, best effort, to anyone it removes).
    fn owner_commit(
        self: &Arc<Self>,
        st: &ServerState,
        next: Manifest,
        key: &ServerKey,
    ) -> Result<()> {
        let prev = st.manifest.clone();
        self.owner_commit_from(&prev, st, next, key)
    }

    /// [`Self::owner_commit`] reporting changes relative to `prev`.
    fn owner_commit_from(
        self: &Arc<Self>,
        prev: &Manifest,
        st: &ServerState,
        next: Manifest,
        key: &ServerKey,
    ) -> Result<()> {
        let signed = SignedManifest::sign(&next, key)?;
        if check_next(&st.server_id, &st.signed, &signed)? != ManifestCheck::Accept {
            bail!("new manifest does not follow the current one");
        }
        let id = st.server_id.clone();
        let removed = self.removed_pins(prev, &next)?;
        self.store.manifest_put(&id, next.seq, &signed, now_ms())?;
        self.store.ops_prune_below(&id, next.seq)?;
        let new = self
            .server_state(&id)?
            .ok_or_else(|| anyhow!("manifest vanished"))?;
        self.settle(Some(prev), &new)?;
        let wire = signed.to_wire()?;
        self.publish(&id, &new, ServerPayloadKind::ServerManifest, &wire, None);
        for pin in removed {
            self.send_direct(pin, &id, ServerPayloadKind::ServerManifest, &wire);
        }
        Ok(())
    }

    /// Non-owner: sign an op on the current manifest, apply it locally and
    /// send it to every member except `skip`.
    fn submit_op(self: &Arc<Self>, st: ServerState, op: OpKind, skip: Option<&str>) -> Result<()> {
        let now = now_ms();
        let rop = RosterOp {
            server_id: st.server_id.clone(),
            base_seq: st.base.seq,
            op,
            ts: now,
            signer: self.me(),
            signer_ed: self.id.ed25519_b64(),
        };
        let signed = SignedOp::sign(&rop, &self.id.keypair)?;
        let mut next = st.clone();
        next.apply_op(&signed.hash(), rop, now)
            .map_err(|e| anyhow!("not allowed: {e:?}"))?;
        let removed = self.removed_pins(&st.manifest, &next.manifest)?;
        self.store
            .op_put(&st.server_id, st.base.seq, &signed, now)?;
        self.settle(Some(&st.manifest), &next)?;
        let wire = signed.to_wire()?;
        self.publish(
            &st.server_id,
            &next,
            ServerPayloadKind::ServerOp,
            &wire,
            skip,
        );
        for pin in removed {
            self.send_direct(pin, &st.server_id, ServerPayloadKind::ServerOp, &wire);
        }
        Ok(())
    }

    /// Add a joiner: the owner writes a new manifest, others sign an `Admit`
    /// op and send the joiner the manifest and ops it needs.
    fn admit(
        self: &Arc<Self>,
        st: ServerState,
        card: ContactCard,
        invite: SignedInvite,
    ) -> Result<()> {
        let row = self.server_row(&st.server_id)?;
        let joiner = card.peer_id.clone();
        if let Some(key) = Self::owner_key(&row)? {
            let mut next = st.next_manifest();
            next.roster.push(RosterEntry {
                card: roster_card(&card)?,
                role: Role::Member,
                added_by: self.me(),
                added_seq: next.seq,
            });
            return self.owner_commit(&st, next, &key);
        }
        let id = st.server_id.clone();
        self.submit_op(st, OpKind::Admit { card, invite }, Some(&joiner))?;
        let st = self
            .server_state(&id)?
            .ok_or_else(|| anyhow!("manifest vanished"))?;
        self.send_state_to(&joiner, &st)
    }

    /// Queue the current manifest and the ops on it for `peer`.
    fn send_state_to(self: &Arc<Self>, peer: &str, st: &ServerState) -> Result<()> {
        let id = &st.server_id;
        let meta = EnvelopeMeta::default;
        self.queue_server_payload(
            peer,
            id,
            ServerPayloadKind::ServerManifest,
            &st.signed.to_wire()?,
            meta(),
        )?;
        for op in self.store.ops_on(id, st.base.seq)? {
            self.queue_server_payload(
                peer,
                id,
                ServerPayloadKind::ServerOp,
                &op.to_wire()?,
                meta(),
            )?;
        }
        Ok(())
    }

    fn publish(
        self: &Arc<Self>,
        server_id: &str,
        st: &ServerState,
        kind: ServerPayloadKind,
        data: &[u8],
        skip: Option<&str>,
    ) {
        let me = self.me();
        for e in st.members() {
            let p = &e.card.peer_id;
            if *p == me || Some(p.as_str()) == skip {
                continue;
            }
            if let Err(e) =
                self.queue_server_payload(p, server_id, kind, data, EnvelopeMeta::default())
            {
                tracing::warn!(%server_id, peer = %p, "not sent: {e:#}");
            }
        }
    }

    /// Pins of members `next` drops, taken before their records go.
    fn removed_pins(&self, before: &Manifest, next: &Manifest) -> Result<Vec<Pin>> {
        let me = self.me();
        let mut out = Vec::new();
        for e in &before.roster {
            let p = &e.card.peer_id;
            if *p != me && !next.roster.iter().any(|n| n.card.peer_id == *p) {
                if let Some(pin) = self.pin(p)? {
                    out.push(pin);
                }
            }
        }
        Ok(out)
    }

    /// Best-effort, not queued: tell a removed member (no longer a co-member,
    /// so the outbox would drop it) that it was removed.
    fn send_direct(
        self: &Arc<Self>,
        pin: Pin,
        server_id: &str,
        kind: ServerPayloadKind,
        data: &[u8],
    ) {
        let Ok(net) = self.net_handle() else {
            return;
        };
        let p = Payload::new(
            pin.peer_id.clone(),
            uuid::Uuid::new_v4().to_string(),
            now_ms(),
            PayloadBody::Server {
                server_id: server_id.to_string(),
                kind,
                data: b64(data),
            },
            EnvelopeMeta::default(),
        );
        let me = self.clone();
        tokio::spawn(async move {
            if me.deliver(&net, &pin, &p).await.is_err() {
                tracing::debug!(peer = %pin.peer_id, "removal notice not delivered");
            }
        });
    }

    /// Bring local records in line with `st`: channels and roles, co-member
    /// records, join and removal status, and events.
    fn settle(self: &Arc<Self>, prev: Option<&Manifest>, st: &ServerState) -> Result<()> {
        let id = st.server_id.as_str();
        let me = self.me();
        let row = self.server_row(id)?;
        let roles: Vec<(String, Role)> = st
            .members()
            .iter()
            .map(|e| (e.card.peer_id.clone(), e.role))
            .collect();
        self.store.server_state_replace(id, st.channels(), &roles)?;
        if let Some(mut m) = self.store.get_membership(id)? {
            let h = Some(st.manifest.policy.history.as_str().to_string());
            if m.history_visibility != h {
                m.history_visibility = h;
                self.store.upsert_membership(&m)?;
            }
        }
        let me_in = st.role_of(&me).is_some();
        let joining = self.store.pending_join(id)?.is_some();
        if joining && !me_in {
            return Ok(()); // the Admit op is still on its way
        }
        let want: HashMap<&str, &ContactCard> = if me_in {
            st.members()
                .iter()
                .filter(|e| e.card.peer_id != me)
                .map(|e| (e.card.peer_id.as_str(), &e.card))
                .collect()
        } else {
            HashMap::new()
        };
        let have: HashSet<String> = self
            .store
            .list_co_members(Some(id))?
            .into_iter()
            .map(|r| r.peer_id)
            .collect();
        for (p, card) in &want {
            if !have.contains(*p) {
                if let Err(e) = self.add_co_member(id, (*card).clone()) {
                    tracing::warn!(server_id = %id, peer = %p, "co-member not added: {e:#}");
                }
            }
        }
        for p in have.iter().filter(|p| !want.contains_key(p.as_str())) {
            let _ = self.remove_co_member(id, p);
        }
        if joining {
            self.store.pending_join_remove(id)?;
            self.emit_srv(id, ServerEvent::Joined);
            return Ok(());
        }
        if let Some(prev) = prev {
            for e in st.members() {
                if e.card.peer_id != me
                    && !prev.roster.iter().any(|p| p.card.peer_id == e.card.peer_id)
                {
                    self.emit_srv(
                        id,
                        ServerEvent::MemberJoined {
                            peer_id: e.card.peer_id.clone(),
                        },
                    );
                }
            }
            for e in &prev.roster {
                let p = &e.card.peer_id;
                if *p != me && st.role_of(p).is_none() {
                    let reason = if st.is_banned(p) {
                        MemberLeftReason::Banned
                    } else {
                        MemberLeftReason::Kicked
                    };
                    self.emit_srv(
                        id,
                        ServerEvent::MemberLeft {
                            peer_id: p.clone(),
                            reason,
                        },
                    );
                }
            }
        }
        if !me_in && row.removed.is_none() {
            let (r, reason) = if st.is_banned(&me) {
                (REMOVED_BANNED, MemberLeftReason::Banned)
            } else {
                (REMOVED_KICKED, MemberLeftReason::Kicked)
            };
            self.store.server_set_removed(id, Some(r))?;
            self.emit_srv(id, ServerEvent::Removed { reason });
            return Ok(());
        }
        if me_in && row.removed.is_some() {
            self.store.server_set_removed(id, None)?;
        }
        if prev.is_some() {
            self.emit_srv(id, ServerEvent::Updated { seq: st.base.seq });
        }
        Ok(())
    }
}

#[derive(Clone, Copy)]
enum Moderation {
    Kick,
    Ban,
    Unban,
}

fn clean_display_name(n: Option<String>) -> Result<Option<String>> {
    let Some(n) = n.map(|n| n.trim().to_string()).filter(|n| !n.is_empty()) else {
        return Ok(None);
    };
    if n.chars().count() > sm::MAX_NAME_CHARS || n.chars().any(char::is_control) {
        bail!(
            "display name must be at most {} characters",
            sm::MAX_NAME_CHARS
        );
    }
    Ok(Some(n))
}

fn join_request(r: JoinRequestRow) -> JoinRequest {
    JoinRequest {
        server_id: r.server_id,
        card: r.card,
        invite_id: r.invite_id,
        requested_at_ms: r.ts,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::core::tests::open;

    type Node = (
        Arc<Core>,
        tokio::sync::mpsc::UnboundedReceiver<CoreEvent>,
        tempfile::TempDir,
    );

    /// In-process network: Olm sessions per pair, outbox pumping.
    #[derive(Default)]
    struct Net {
        sessions: HashSet<(String, String)>,
    }

    impl Net {
        fn session(&mut self, from: &Arc<Core>, to: &Arc<Core>) {
            let (f, t) = (from.me(), to.me());
            if from.crypto.has_session(&t).unwrap() {
                return;
            }
            let KeysResponse::Key {
                identity_key,
                one_time_key,
            } = to.handle_keys(from.peer_id(), KeysRequest::ClaimOneTimeKey)
            else {
                panic!("no key from {t}");
            };
            from.crypto
                .create_outbound(
                    &t,
                    Curve25519PublicKey::from_base64(&identity_key).unwrap(),
                    Curve25519PublicKey::from_base64(&one_time_key).unwrap(),
                )
                .unwrap();
            self.sessions.insert((f, t));
        }

        /// Like `Core::deliver`: one fresh session if the peer can't decrypt.
        fn send(&mut self, from: &Arc<Core>, to: &Arc<Core>, p: &Payload) -> MsgResponse {
            let mut r = MsgResponse::Rejected(RejectReason::DecryptFailed);
            for _ in 0..2 {
                self.session(from, to);
                let (t, c) = from
                    .crypto
                    .encrypt(&to.me(), &encode_payload(p).unwrap())
                    .unwrap();
                r = to.handle_msg(from.peer_id(), MsgRequest::Olm { t, c });
                if r != MsgResponse::Rejected(RejectReason::DecryptFailed) {
                    break;
                }
                from.crypto.forget_sessions(&to.me()).unwrap();
            }
            r
        }

        /// Deliver everything `from` has queued for `to`. Returns the count.
        fn pump(&mut self, from: &Arc<Core>, to: &Arc<Core>) -> usize {
            let rows = from.store.outbox_for(&to.me()).unwrap();
            let n = rows.len();
            for row in rows {
                let OutboxItem::Server { payload, .. } = &row.item else {
                    continue;
                };
                let p: Payload = serde_json::from_slice(payload).unwrap();
                assert_eq!(self.send(from, to, &p), MsgResponse::Delivered);
                from.store.outbox_remove(&row.msg_id).unwrap();
            }
            n
        }

        /// Pump between all nodes until nothing is queued.
        fn settle(&mut self, nodes: &[&Arc<Core>]) {
            loop {
                let mut n = 0;
                for a in nodes {
                    for b in nodes {
                        if a.me() != b.me() {
                            n += self.pump(a, b);
                        }
                    }
                }
                if n == 0 {
                    return;
                }
            }
        }
    }

    fn join(
        net: &mut Net,
        joiner: &Arc<Core>,
        inviter: &Arc<Core>,
        link: &str,
    ) -> Result<ServerInfo> {
        let (sid, _, _, req) = joiner.prepare_join(link, None)?;
        let resp = inviter.handle_msg(joiner.peer_id(), req);
        let _ = net;
        joiner.finish_join(&sid, Ok(resp))
    }

    fn reject_of(joiner: &Arc<Core>, inviter: &Arc<Core>, link: &str) -> MsgResponse {
        let (sid, _, _, req) = joiner.prepare_join(link, None).unwrap();
        let resp = inviter.handle_msg(joiner.peer_id(), req);
        let _ = joiner.finish_join(&sid, Ok(resp.clone()));
        resp
    }

    fn server_events(rx: &mut tokio::sync::mpsc::UnboundedReceiver<CoreEvent>) -> Vec<ServerEvent> {
        let mut out = Vec::new();
        while let Ok(e) = rx.try_recv() {
            if let CoreEvent::Server { event, .. } = e {
                out.push(event);
            }
        }
        out
    }

    fn invite(core: &Arc<Core>, sid: &str, opts: InviteOptions) -> String {
        core.create_invite(sid, opts).unwrap().link
    }

    impl Core {
        fn membership_identity(&self, sid: &str) -> String {
            self.store.get_membership(sid).unwrap().unwrap().identity
        }
    }

    fn seq(core: &Arc<Core>, sid: &str) -> u64 {
        core.server_info(sid).unwrap().seq
    }

    #[tokio::test]
    async fn create_server_holds_the_owner_key_and_a_recovery_key() {
        let (a, _rx, _d): Node = open(1);
        let c = a.create_server("Den", vec![], None).unwrap();
        let s = &c.server;
        assert!(c.recovery_secret.is_some());
        assert_eq!(s.name, "Den");
        assert_eq!(s.my_role, Some(ServerRole::Owner));
        assert_eq!(s.status, ServerStatus::Active);
        assert!(s.is_owner_device);
        assert_eq!((s.seq, s.member_count, s.channel_count), (0, 1, 1));
        assert_eq!(s.invite_roles, vec![ServerRole::Owner, ServerRole::Admin]);
        assert!(s.history_shared);
        assert_eq!(s.retention_days, 30);
        assert_eq!(s.owner_fingerprint.len(), 39);
        assert_eq!(a.list_channels(&s.server_id).unwrap()[0].name, "general");
        assert!(a.server_membership(&s.server_id).unwrap().is_some());
        // Supplied recovery keys are used as is (and validated).
        let r = ServerKey::generate().public_b64();
        let c2 = a.create_server("Two", vec![r], None).unwrap();
        assert!(c2.recovery_secret.is_none());
        assert!(a.create_server("Bad", vec!["nope".into()], None).is_err());
        assert!(a.create_server("", vec![], None).is_err());
        assert_eq!(a.list_servers().unwrap().len(), 2);
        // Owner key and server key secret never appear in the FFI types.
        let ch = a.create_channel(&s.server_id, "memes").unwrap();
        assert!(ch.can_post && ch.can_read);
        assert_eq!(seq(&a, &s.server_id), 1);
    }

    #[tokio::test]
    async fn invite_join_admin_admit_and_owner_fold() {
        let mut net = Net::default();
        let (a, _ra, _da): Node = open(1);
        let (b, mut rb, _db): Node = open(2);
        let (c, mut rc, _dc): Node = open(3);
        let sid = a
            .create_server("Den", vec![], None)
            .unwrap()
            .server
            .server_id;

        // Owner invites B: owner path, new manifest.
        let link = invite(&a, &sid, InviteOptions::default());
        let pv = b.parse_invite(&link).unwrap();
        assert_eq!(pv.server_name, "Den");
        assert_eq!(
            pv.owner_fingerprint,
            a.server_info(&sid).unwrap().owner_fingerprint
        );
        assert!(!pv.expired && !pv.already_member);
        let info = join(&mut net, &b, &a, &link).unwrap();
        assert_eq!(info.status, ServerStatus::Joining);
        net.settle(&[&a, &b]);
        assert!(server_events(&mut rb).contains(&ServerEvent::Joined));
        let bi = b.server_info(&sid).unwrap();
        assert_eq!(
            (bi.status, bi.my_role, bi.seq),
            (ServerStatus::Active, Some(ServerRole::Member), 1)
        );
        assert!(!bi.is_owner_device);
        // One use only.
        assert_eq!(
            reject_of(&c, &a, &link),
            MsgResponse::Rejected(RejectReason::InviteInvalid)
        );

        // Members can't invite by default; the owner promotes B.
        assert!(b.create_invite(&sid, InviteOptions::default()).is_err());
        assert!(b.set_role(&sid, &a.me(), ServerRole::Member).is_err());
        a.set_role(&sid, &b.me(), ServerRole::Admin).unwrap();
        net.settle(&[&a, &b]);
        assert_eq!(
            b.server_info(&sid).unwrap().my_role,
            Some(ServerRole::Admin)
        );

        // Admin B admits C with an Admit op; the owner folds it.
        let link = invite(
            &b,
            &sid,
            InviteOptions {
                max_uses: Some(2),
                ..Default::default()
            },
        );
        join(&mut net, &c, &b, &link).unwrap();
        assert_eq!(b.list_members(&sid).unwrap().len(), 3, "op applied at once");
        net.settle(&[&a, &b, &c]);
        assert!(server_events(&mut rc).contains(&ServerEvent::Joined));
        for n in [&a, &b, &c] {
            let i = n.server_info(&sid).unwrap();
            assert_eq!(
                (i.seq, i.member_count, i.status),
                (3, 3, ServerStatus::Active),
                "{}",
                n.me()
            );
        }
        assert!(server_events(&mut rb)
            .iter()
            .any(|e| matches!(e, ServerEvent::MemberJoined { peer_id } if *peer_id == c.me())));
        assert_eq!(
            c.server_info(&sid).unwrap().my_role,
            Some(ServerRole::Member)
        );
        let co = c.list_co_members(Some(&sid)).unwrap();
        assert_eq!(co.len(), 2, "C pins A and B from the roster");
        assert!(co.iter().all(|m| !m.is_contact && m.signed_card));

        // Admin creates a channel by op, renames it; the owner folds both.
        let ch = b.create_channel(&sid, "news").unwrap();
        b.rename_channel(&sid, &ch.id, "announcements").unwrap();
        net.settle(&[&a, &b, &c]);
        let names: Vec<String> = c
            .list_channels(&sid)
            .unwrap()
            .into_iter()
            .map(|c| c.name)
            .collect();
        assert_eq!(names, vec!["general", "announcements"]);
        assert_eq!(seq(&a, &sid), seq(&c, &sid));

        // C, a member, can't moderate or make channels.
        assert!(c.kick(&sid, &b.me()).is_err());
        assert!(c.create_channel(&sid, "x").is_err());
        // Admins can't act on the owner or other admins.
        assert!(b.kick(&sid, &a.me()).is_err());
        // Only the owner changes roles and invite roles.
        assert!(b.set_invite_roles(&sid, vec![ServerRole::Member]).is_err());
        a.set_invite_roles(&sid, vec![ServerRole::Member]).unwrap();
        net.settle(&[&a, &b, &c]);
        let i = c.server_info(&sid).unwrap();
        assert_eq!(i.invite_roles, vec![ServerRole::Owner, ServerRole::Member]);
        assert!(
            b.create_invite(&sid, InviteOptions::default()).is_err(),
            "admins lost invite"
        );
        assert!(c.create_invite(&sid, InviteOptions::default()).is_ok());
    }

    #[tokio::test]
    async fn history_defaults_to_shared_and_names_are_per_server() {
        let mut net = Net::default();
        let (a, _ra, _da): Node = open(1);
        let (b, _rb, _db): Node = open(2);
        let sid = a
            .create_server("Den", vec![], Some("Boss".into()))
            .unwrap()
            .server
            .server_id;
        let l = invite(&a, &sid, InviteOptions::default());
        let info = b
            .prepare_join(&l, Some("  Kit ".into()))
            .and_then(|(s, _, _, req)| b.finish_join(&s, Ok(a.handle_msg(b.peer_id(), req))))
            .unwrap();
        assert_eq!(info.my_display_name.as_deref(), Some("Kit"));
        net.settle(&[&a, &b]);
        // On by default, per server, owner only.
        let hist = |n: &Arc<Core>| n.server_info(&sid).unwrap().history_shared;
        assert!(hist(&a) && hist(&b));
        assert_eq!(
            b.server_membership(&sid)
                .unwrap()
                .unwrap()
                .history_visibility,
            Some(HistoryVisibility::Shared)
        );
        assert!(b.set_history_shared(&sid, false).is_err());
        a.set_history_shared(&sid, false).unwrap();
        net.settle(&[&a, &b]);
        assert!(!hist(&b));
        assert_eq!(
            b.server_membership(&sid)
                .unwrap()
                .unwrap()
                .history_visibility,
            Some(HistoryVisibility::Joined)
        );
        // Same identity, per-server names.
        let names = |n: &Arc<Core>| -> Vec<(String, String)> {
            n.list_members(&sid)
                .unwrap()
                .into_iter()
                .map(|m| (m.peer_id, m.display_name))
                .collect()
        };
        assert!(names(&b).contains(&(a.me(), "Boss".into())));
        assert!(names(&a).contains(&(b.me(), "Kit".into())));
        a.set_server_display_name(&sid, Some("Chief".into()))
            .unwrap();
        net.settle(&[&a, &b]);
        assert!(names(&b).contains(&(a.me(), "Chief".into())));
        assert_eq!(b.membership_identity(&sid), "default");
        assert!(a
            .set_server_display_name(&sid, Some("x".repeat(500)))
            .is_err());
    }

    #[tokio::test]
    async fn kicks_bans_and_join_refusals() {
        let mut net = Net::default();
        let (a, _ra, _da): Node = open(1);
        let (b, _rb, _db): Node = open(2);
        let (c, mut rc, _dc): Node = open(3);
        let (d, _rd, _dd): Node = open(4);
        let sid = a
            .create_server("Den", vec![], None)
            .unwrap()
            .server
            .server_id;
        for n in [&b, &c] {
            let l = invite(&a, &sid, InviteOptions::default());
            join(&mut net, n, &a, &l).unwrap();
        }
        net.settle(&[&a, &b, &c]);
        a.set_role(&sid, &b.me(), ServerRole::Admin).unwrap();
        net.settle(&[&a, &b, &c]);
        assert_eq!(c.server_info(&sid).unwrap().member_count, 3);

        // A bad proof, an unknown invite, a revoked invite.
        let l = invite(&a, &sid, InviteOptions::default());
        let (_, _, _, req) = d.prepare_join(&l, None).unwrap();
        let MsgRequest::JoinHello {
            card,
            server_id,
            invite_id,
            ..
        } = req
        else {
            panic!()
        };
        let bad = MsgRequest::JoinHello {
            card: card.clone(),
            server_id: server_id.clone(),
            invite_id: invite_id.clone(),
            proof: vec![0; 32],
        };
        assert_eq!(
            a.handle_msg(d.peer_id(), bad),
            MsgResponse::Rejected(RejectReason::InviteInvalid)
        );
        // The proof binds the joiner: B can't replay D's proof.
        let (_, _, _, req) = d.prepare_join(&l, None).unwrap();
        let MsgRequest::JoinHello { proof, .. } = req else {
            panic!()
        };
        assert_eq!(
            a.handle_msg(
                b.peer_id(),
                MsgRequest::JoinHello {
                    card: b.self_card(),
                    server_id: server_id.clone(),
                    invite_id: invite_id.clone(),
                    proof,
                }
            ),
            MsgResponse::Rejected(RejectReason::InviteInvalid)
        );
        a.revoke_invite(&invite_id).unwrap();
        assert_eq!(
            reject_of(&d, &a, &l),
            MsgResponse::Rejected(RejectReason::InviteInvalid)
        );
        assert!(
            d.store.server_get(&sid).unwrap().is_none(),
            "refusal undoes the join"
        );

        // Admin B bans C by op; the owner folds it; C learns from the manifest.
        b.ban(&sid, &c.me()).unwrap();
        net.settle(&[&a, &b]);
        assert_eq!(a.server_info(&sid).unwrap().member_count, 2);
        let wire = a
            .store
            .manifest_latest(&sid)
            .unwrap()
            .unwrap()
            .to_wire()
            .unwrap();
        let p = Payload::new(
            c.me(),
            "m".into(),
            now_ms(),
            PayloadBody::Server {
                server_id: sid.clone(),
                kind: ServerPayloadKind::ServerManifest,
                data: b64(&wire),
            },
            EnvelopeMeta::default(),
        );
        assert_eq!(net.send(&a, &c, &p), MsgResponse::Delivered);
        let ci = c.server_info(&sid).unwrap();
        assert_eq!(
            (ci.status, ci.removed_reason, ci.my_role),
            (ServerStatus::Removed, Some(MemberLeftReason::Banned), None)
        );
        assert!(server_events(&mut rc).contains(&ServerEvent::Removed {
            reason: MemberLeftReason::Banned
        }));
        assert!(c.list_co_members(Some(&sid)).unwrap().is_empty());
        assert!(c.create_channel(&sid, "x").is_err());

        // Banned: a fresh invite doesn't help. (Ban beats admit.)
        let l = invite(&a, &sid, InviteOptions::default());
        assert_eq!(
            reject_of(&c, &a, &l),
            MsgResponse::Rejected(RejectReason::Banned)
        );
        a.unban(&sid, &c.me()).unwrap();
        let l = invite(&a, &sid, InviteOptions::default());
        join(&mut net, &c, &a, &l).unwrap();
        net.settle(&[&a, &b, &c]);
        assert_eq!(c.server_info(&sid).unwrap().status, ServerStatus::Active);

        // Owner kicks C.
        a.kick(&sid, &c.me()).unwrap();
        net.settle(&[&a, &b]);
        assert!(b
            .list_members(&sid)
            .unwrap()
            .iter()
            .all(|m| m.peer_id != c.me()));

        // B is demoted: its old invite no longer admits anyone.
        a.set_invite_roles(&sid, vec![ServerRole::Admin]).unwrap();
        net.settle(&[&a, &b]);
        let l = invite(&b, &sid, InviteOptions::default());
        a.set_role(&sid, &b.me(), ServerRole::Member).unwrap();
        net.settle(&[&a, &b]);
        assert_eq!(
            reject_of(&d, &b, &l),
            MsgResponse::Rejected(RejectReason::NotAuthorized)
        );

        // JoinHello has its own bucket.
        let l = invite(&a, &sid, InviteOptions::default());
        let mut limited = false;
        for _ in 0..10 {
            let (_, _, _, req) = d.prepare_join(&l, None).unwrap();
            let r = a.handle_msg(d.peer_id(), req);
            limited |= r == MsgResponse::Rejected(RejectReason::RateLimited);
        }
        assert!(limited);
    }

    #[tokio::test]
    async fn approval_invites_wait_for_the_inviter() {
        let mut net = Net::default();
        let (a, mut ra, _da): Node = open(1);
        let (b, _rb, _db): Node = open(2);
        let sid = a
            .create_server("Den", vec![], None)
            .unwrap()
            .server
            .server_id;
        let l = invite(
            &a,
            &sid,
            InviteOptions {
                needs_approval: true,
                ..Default::default()
            },
        );
        assert!(b.parse_invite(&l).unwrap().needs_approval);
        join(&mut net, &b, &a, &l).unwrap();
        net.settle(&[&a, &b]);
        assert_eq!(b.server_info(&sid).unwrap().status, ServerStatus::Joining);
        assert!(server_events(&mut ra).iter().any(
            |e| matches!(e, ServerEvent::JoinRequest { request } if request.card.peer_id == b.me())
        ));
        assert_eq!(a.join_requests(&sid).unwrap().len(), 1);
        a.approve_join(&sid, &b.me(), true).unwrap();
        assert!(a.join_requests(&sid).unwrap().is_empty());
        net.settle(&[&a, &b]);
        assert_eq!(b.server_info(&sid).unwrap().status, ServerStatus::Active);
        assert!(a.approve_join(&sid, &b.me(), true).is_err());
        // B leaves: everything about the server is gone on B.
        b.leave_server(&sid).unwrap();
        assert!(b.store.server_get(&sid).unwrap().is_none());
        assert!(b.list_co_members(Some(&sid)).unwrap().is_empty());
        assert!(b.server_membership(&sid).unwrap().is_none());
        // The owner can't leave while others remain.
        assert!(a.leave_server(&sid).is_err());
    }

    #[tokio::test]
    async fn rollbacks_are_refused_and_forks_freeze_the_server() {
        let mut net = Net::default();
        let (a, _ra, _da): Node = open(1);
        let (b, mut rb, _db): Node = open(2);
        let sid = a
            .create_server("Den", vec![], None)
            .unwrap()
            .server
            .server_id;
        let l = invite(&a, &sid, InviteOptions::default());
        join(&mut net, &b, &a, &l).unwrap();
        a.create_channel(&sid, "two").unwrap();
        net.settle(&[&a, &b]);
        assert_eq!(seq(&b, &sid), 2);
        let send_manifest = |net: &mut Net, m: &SignedManifest| {
            let p = Payload::new(
                b.me(),
                uuid::Uuid::new_v4().to_string(),
                now_ms(),
                PayloadBody::Server {
                    server_id: sid.clone(),
                    kind: ServerPayloadKind::ServerManifest,
                    data: b64(&m.to_wire().unwrap()),
                },
                EnvelopeMeta::default(),
            );
            net.send(&a, &b, &p)
        };
        // Replaying seq 1 is a rollback.
        let old = a.store.manifest_at(&sid, 1).unwrap().unwrap();
        send_manifest(&mut net, &old);
        server_events(&mut rb);
        assert_eq!(seq(&b, &sid), 2);
        send_manifest(&mut net, &old);
        assert!(server_events(&mut rb).contains(&ServerEvent::ManifestRollback { have: 2, got: 1 }));
        // A second, different seq 2 signed by K_S is a fork: frozen.
        let row = a.store.server_get(&sid).unwrap().unwrap();
        let key = Core::owner_key(&row).unwrap().unwrap();
        let mut m = a
            .store
            .manifest_at(&sid, 2)
            .unwrap()
            .unwrap()
            .manifest()
            .unwrap();
        m.name = "Evil".into();
        let forked = SignedManifest::sign(&m, &key).unwrap();
        send_manifest(&mut net, &forked);
        assert!(server_events(&mut rb).contains(&ServerEvent::Forked { seq: 2 }));
        let bi = b.server_info(&sid).unwrap();
        assert_eq!((bi.status, bi.name.as_str()), (ServerStatus::Frozen, "Den"));
        assert_eq!(b.store.manifest_forks(&sid).unwrap().len(), 1);
        // A manifest not signed by K_S is ignored.
        let mut m3 = m.clone();
        m3.seq = 3;
        let other = SignedManifest::sign(&m3, &ServerKey::generate()).unwrap();
        send_manifest(&mut net, &other);
        assert_eq!(seq(&b, &sid), 2);
    }
}
