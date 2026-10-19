# Umbra core API (`core/`, crate `umbra_core`)

Rust core exposed to Flutter through **flutter_rust_bridge v2**.

| Item | Value |
| --- | --- |
| Crate | `core/` (`umbra_core`), workspace root `core/Cargo.toml` |
| `crate-type` | `cdylib`, `staticlib`, `rlib` (cargokit-ready) |
| frb Rust crate | `flutter_rust_bridge = "=2.13.0"` |
| frb codegen / Dart package | **must be exactly 2.13.0** (`cargo install flutter_rust_bridge_codegen --version 2.13.0`, `flutter_rust_bridge: 2.13.0` in pubspec) |
| frb `rust_input` | `crate::api` (files in `core/src/api/`) |
| frb `rust_root` | `../core/` relative to `app/` |
| Toolchain | Rust >= 1.89 (vodozemac 0.11). `core/rust-toolchain.toml` pins 1.99.1 for rustup users |
| libp2p | 0.55 (pinned in `[workspace.dependencies]` to match `umbra-tor`) |
| SQLite | `rusqlite 0.37` + `bundled-sqlcipher`; Android/iOS/Windows automatically use `bundled-sqlcipher-vendored-openssl` (feature `vendored-openssl` forces it elsewhere) |

`core/src/frb_generated.rs` is a **placeholder** so the crate compiles before
codegen; `flutter_rust_bridge_codegen generate` overwrites it.

## Status

Real backend implemented (identity, SQLCipher store, Olm, libp2p, outbox,
Tor-only mode). `CoreConfig { mock: true }` still gives the in-memory mock
backend for UI work.

## Building

```bash
export PATH="$HOME/.cargo/bin:$PATH"     # rustup toolchain, not /usr/bin (1.85)
cd core
cargo build && cargo test && cargo clippy --all-targets
```

Android (verified for aarch64 / armv7 / x86_64 with NDK r27c, API 24):

```bash
N=$ANDROID_NDK_HOME/toolchains/llvm/prebuilt/linux-x86_64/bin
export CC_aarch64_linux_android=$N/aarch64-linux-android24-clang \
       AR_aarch64_linux_android=$N/llvm-ar \
       CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER=$N/aarch64-linux-android24-clang
cargo build --release --lib --target aarch64-linux-android
```

Needs `perl` and `make` on the build host (OpenSSL is built from source for
SQLCipher). The resulting `.so` only links `libc`/`libdl`. cargokit sets the
CC/linker variables itself.

## Lifecycle

```rust
async fn init(data_dir: String, db_key: Vec<u8>, cfg: CoreConfig) -> Result<SelfInfo>;
async fn start_network() -> Result<()>;
async fn stop_network() -> Result<()>;
fn events(sink: StreamSink<CoreEvent>) -> Result<()>;   // single stream
fn network_status() -> Result<NetworkInfo>;              // ADDITION
```

* `db_key`: exactly **32 random bytes**, generated once by the app and kept in
  the OS keystore (Android Keystore / Windows DPAPI / libsecret). Core uses it
  as the raw SQLCipher key and to derive the Olm pickle key; it is never
  written to disk by core. A wrong key makes `init` fail.
* `init` is idempotent for the same `data_dir` (Flutter hot restart).
* Subscribe to `events` once, early (before `start_network`). Calling it again
  replaces the previous stream.

## Identity & contacts

```rust
fn self_info() -> Result<SelfInfo>;
fn set_display_name(name: String) -> Result<()>;
fn parse_qr(payload: String) -> Result<ContactCard>;      // works before init
async fn add_contact(card: ContactCard, verified: bool) -> Result<Contact>;
fn list_contacts() -> Result<Vec<Contact>>;
fn set_verified(peer_id: String, verified: bool) -> Result<()>;
fn remove_contact(peer_id: String) -> Result<()>;
fn accept_key_change(peer_id: String) -> Result<Contact>;   // ADDITION (security)
fn reject_key_change(peer_id: String) -> Result<()>;        // ADDITION (security)
```

QR payload:

```
umbra://add?peer=<PeerId>&ik=<curve25519 b64>&ed=<ed25519 b64>&addr=<ma1>,<ma2>[&name=<display name>][&sig=<b64>]
```

`parse_qr` checks that `ed` derives `peer`, that `ik` is a valid
Curve25519 key and, if `sig` is present, that it is a valid card v2
signature (see below). Base64 is standard alphabet, unpadded (URL-encoded in the
query). At most 6 addresses are written.

* `SelfInfo.fingerprint`: our 30-digit half (6 groups of 5).
* `Contact.safety_number`: 60 digits (12 groups of 5), identical on both
  devices. Show it on the contact screen and let the user mark `verified`.

## Signed contact cards (card v2)

Per [servers.md](servers.md) prerequisite 1, a card binds the Olm identity
key to the device key:

```
sig = ed25519_sign(ed, "umbra-card-v2" || PeerId bytes || ik)
```

* `PeerId bytes` is the binary PeerId (multihash, `PeerId::to_bytes`), `ik`
  the 32 raw Curve25519 bytes. No length prefixes (the multihash is
  self-delimiting, `ik` is fixed-size). `sig` is 64 bytes, unpadded base64,
  in `ContactCard.sig` and the QR's `sig=`.
* Core signs its own card (`self_info().qr_payload`, every `Hello`).
* Every card core accepts (`parse_qr`, `add_contact`, `Hello`) is verified
  when signed. **An invalid signature rejects the card**: `add_contact`
  errors, `Hello` gets `Rejected(Malformed)`, and no `KeyChanged` is raised
  (a forged card can't trigger key-change alerts).
* **Compatibility:** unsigned (v1) cards are still accepted for contacts.
  `Contact.signed_card` says whether we hold a valid signature; legacy
  contacts become signed on their next signed `Hello` (same `ik`).
* **No downgrade:** once a contact is signed, an unsigned card for it is
  refused (`Rejected(Malformed)` / `add_contact` error).
* **With KeyChanged:** a validly signed card with a *different* `ik` is
  still only a key change: never auto-trusted, same flow as below. Its
  signature is kept with the pending key and becomes the contact's card
  signature on `accept_key_change`. A key change seen only in a key-claim
  answer has no signature, so the contact is unsigned after accepting it
  until the next signed `Hello`.
* Schema v3 adds `contacts.card_sig` and `contacts.pending_card_sig`.

## Identity key changes

A known contact (same PeerId, i.e. same Ed25519 device key, which Noise
authenticates) can present a **different Olm identity key**: in its `Hello`,
in its answer to a one-time-key claim, or in a re-scanned QR passed to
`add_contact`. Core never trusts it automatically:

1. The new key is **rejected** (`Hello` gets `Rejected(KeyMismatch)`, the
   key claim is not used, `add_contact` returns an error). Messages keep
   using the old, trusted key.
2. The contact is flagged: `Contact.key_changed = true`,
   `Contact.pending_identity_key = Some(new)`.
3. `CoreEvent::KeyChanged { peer_id, old_identity_key, new_identity_key,
   old_safety_number, new_safety_number, source }` is emitted **once per
   distinct new key** (`source`: `Hello | KeyClaim | ContactCard`).

The app should show a blocking warning and ask the user to compare the
`new_safety_number` in person, then call one of:

* `accept_key_change(peer_id)`: the pending key becomes the identity key,
  old Olm sessions are dropped, `verified` is set to **false**, the flag is
  cleared, and queued messages are re-sent under the new key.
* `reject_key_change(peer_id)`: keep the old key. The refused key is
  remembered and is rejected silently from then on; a *different* new key
  alerts again.

Both return an error if nothing is pending. The DB schema moved to v2
(columns `pending_identity_key`, `rejected_identity_key`, `key_changed_at`);
v1 databases are migrated on `init`.

## Servers, Phase 0: memberships and co-members (engine API)

Phase 0 of [servers.md](servers.md). These are **engine-level** (`Core`)
methods for Phase 1 server code in core. They are **not in the FFI** yet
(no Dart bindings), so the app can't use them directly.

```rust
// Local membership: which of OUR identities this server uses (explicit).
fn add_server_membership(&self, server_id: &str, identity: ServerIdentity,
                         history: Option<HistoryVisibility>) -> Result<ServerMembership>;
fn server_membership(&self, server_id: &str) -> Result<Option<ServerMembership>>;
fn set_server_history(&self, server_id: &str, history: Option<HistoryVisibility>) -> Result<()>;
// Co-members (from roster entries; Phase 1 calls these).
fn add_co_member(self: &Arc<Self>, server_id: &str, card: ContactCard) -> Result<CoMember>;
fn remove_co_member(&self, server_id: &str, peer_id: &str) -> Result<()>;
fn co_member(&self, server_id: &str, peer_id: &str) -> Result<Option<CoMember>>;
fn list_co_members(&self, server_id: Option<&str>) -> Result<Vec<CoMember>>;
fn accept_key_change_any(self: &Arc<Self>, peer_id: &str) -> Result<()>; // contacts or co-members
fn subscribe_server(&self) -> UnboundedReceiver<ServerInbound>;           // engine-internal

enum ServerIdentity { Default, Dedicated }   // Dedicated: refused in Phase 0
enum HistoryVisibility { Joined, Shared }    // stored only; None = unset
struct ServerMembership { server_id, identity, history_visibility: Option<_>, joined_at_ms }
struct CoMember { server_id, peer_id, display_name, identity_key, ed25519_key, addrs,
                  added_at_ms, signed_card, key_changed, pending_identity_key, is_contact }
struct ServerInbound { peer_id, server_id, kind: ServerPayloadKind, data: Vec<u8>, id, ts }
```

**Identity per server.** Every membership records which of our identities
it uses, as an explicit parameter with no default: `Default` (the user's
normal PeerId and keys) or `Dedicated` (a separate identity for that server).
`Dedicated` is accepted by the API but refused in Phase 0, because it needs
its own device key, Olm account and network identity. Rules for when it
lands: a dedicated identity is never linked to the default one in cards,
`Hello`s, logs or outbox rows, and is never used toward contacts. A
membership's identity can't change. Other members may also use per-server
identities. To us that is simply a different PeerId, so co-member records are
keyed by **(server_id, member PeerId)**. The PeerId is derived from the
member's Ed25519 identity key, so this is the same as keying by that key.

**History visibility** is a per-server setting (`HistoryVisibility`,
`None` = not set). Phase 0 only stores it, and nothing in core hard-wires
"from join".

**Co-member trust class.** A co-member is a peer from a server roster with
a **signed (v2) card**. It is stored separately from contacts (table
`co_members`), never shows up in `list_contacts`, and is **never promoted to a
contact automatically**. Only `add_contact` (a user action) makes a contact.

| From a co-member who isn't a contact | Result |
|---|---|
| `Hello` (must carry a valid v2 card) | `Ok`, no `ContactRequest`; unsigned → `Malformed` |
| `ClaimOneTimeKey` | Served, with the **same per-peer limits** as contacts (claim bucket, outstanding-OTK caps) |
| Olm `Server { server_id, .. }` | Delivered to `subscribe_server` only if they are a co-member of **that** `server_id`; else `Rejected(NotAMember)` |
| Olm `Text` | `Rejected(NotAContact)`, nothing stored. It raises `ContactRequest { card }` (rate-limited like any contact request), matching servers.md's "handled like a contact request" (open question 6) |
| Olm `Signal` (calls) | `Rejected(NotAContact)` |

Strangers are unchanged: contact requests, and no keys.

**Pins and KeyChanged.** A peer has a single pin across its contact record and
all of its co-member records (the contact's is authoritative). The pin is
checked on `add_co_member`, `add_contact`, `Hello` and key claims:

* A different key raises `KeyChanged` once and marks **every** record of the
  peer as pending. Roster mismatches use the new `KeyChangeSource::Roster`.
* While a change is pending, or if any copy disagrees or the record is
  unsigned, **all server traffic to and from the peer stops**
  (`NotAMember`). DMs with a contact behave as before.
* Co-member records only accept **signed** new keys. An unsigned change of a
  co-member-only peer (a key claim answer) is refused without raising an alert.
* `accept_key_change_any` / `reject_key_change` resolve all records of the
  peer together. FFI `accept_key_change` is still contacts-only, so a
  co-member-only key change can't be accepted from the app yet (Phase 1
  members API). `reject_key_change` works for both.
* `remove_contact` keeps the Olm sessions while the peer is still a co-member.
  `remove_co_member` drops them once the peer is in no shared server and
  isn't a contact.

### Generalized outbox and envelope fields (Phase 0)

```rust
fn queue_server_payload(self: &Arc<Self>, peer_id: &str, server_id: &str, kind: ServerPayloadKind,
                        data: &[u8], meta: EnvelopeMeta) -> Result<String /* payload id */>;
struct EnvelopeMeta { seq: Option<u64>, prev: Option<String>, expires_at: Option<i64> }
```

* The outbox holds **typed items**: `Dm` (the text stays in `messages`, as
  before) or `Server { server_id, payload }`, where the payload is the full
  Olm plaintext for one co-member. Both kinds share the same ordering,
  retries, flush triggers (connect, `Hello`, discovery, the 15 s tick) and the
  **same 7-day expiry**. An expired DM is still marked `Failed`. An expired
  server item is dropped without a `MessageStatus` (Phase 1 decides what, if
  anything, to show).
* A server item is sent only while the peer is a signed co-member of that
  server with no pending key change. It is held while a change is pending,
  and dropped if the peer leaves that server (`remove_co_member`).
  `remove_contact` drops DM items only. Each row records which of our
  identities sends it (`local_identity`, always `"default"` in Phase 0).
* **Reserved envelope fields** (Jal is designing what they mean): every Olm
  `Payload` may carry, inside the encryption,
  * `seq`: a per-sender sequence number that only increases, for detecting
    gaps and deletions;
  * `prev`: the hash of the sender's previous message in the stream (a hash
    chain), base64, at most 128 chars;
  * `expires_at`: expiry requested by the sender, in ms since the epoch, for
    disappearing messages.

  All three are optional. They are left out of the JSON when unset, so a
  payload without them is byte-identical to the old format. Older cores
  ignore them (serde skips unknown fields), and future unknown fields are
  ignored too. Core stores them with queued items (`outbox.seq / prev_hash /
  expires_at`) and with received DMs (`messages.seq / prev_hash /
  expires_at`), and passes them on in `ServerInbound.meta`. Core numbers
  outgoing DMs with `seq` and checks incoming DM `seq` (see *Anti-replay*
  below); it doesn't check `prev` chains or delete expired messages yet
  (Phase 1). A `prev` over 128 chars is `Rejected(Malformed)`. They aren't
  in the FFI `Message` type yet.
* Schema v3 (migrated in one transaction, columns added only if missing):
  `contacts.card_sig / pending_card_sig`, the tables `server_memberships`
  and `co_members`, `outbox.kind / server_id / payload / seq / prev_hash /
  expires_at / local_identity`, and `messages.seq / prev_hash / expires_at`.
  Schema v4 adds the anti-replay tables `group_seen`, `seq_seen` and
  `seq_out` (new tables only). servers.md's Phase 1 tables ("schema v3"
  there) are v5; see *Servers, Phase 1*.

### Anti-replay (Phase 0, [message-protection.md](message-protection.md#anti-replay))

```rust
fn accept_group_message(&self, session_id: &str, message_index: u32) -> Result<bool>;
fn discard_group_session(&self, session_id: &str) -> Result<()>;
fn check_sequence(&self, stream: &str, sender: &str, seq: u64, digest: &[u8]) -> Result<SeqVerdict>;
enum SeqVerdict { Fresh, Duplicate, Fork, Stale }
```

* **Megolm** (hook only; Phase 0 has no group sessions): vodozemac's
  `InboundGroupSession::decrypt` re-decrypts any index, so Phase 1 must call
  `accept_group_message` after decrypting and before delivering. It records
  `(session_id, message_index)` persistently (table `group_seen`) and returns
  `false` for a replay. `discard_group_session` prunes the record when the
  session is deleted.
* **DMs**: `send_text` gives every DM a per-peer `seq` (1, 2, 3, ...; stream
  `dm:<peer>` in `seq_out`), stored with the message and the outbox row.
  On receipt, a DM with a `seq`:
  * above everything seen from that peer is delivered and recorded;
  * equal to a recorded one with the same content (SHA-256 of id, ts and
    body) is a retransmission: acked `Delivered`, not delivered again;
  * equal to a recorded one with different content is a **sender fork** (or
    the sender's state was rolled back): logged at warn and
    `Rejected(Replayed)`;
  * otherwise at or below the highest seen is a replay: `Rejected(Replayed)`.

  The last 1024 seqs per peer are kept, plus the highest. DMs without `seq`
  (older cores) keep the `MessageId` dedupe only. The records survive
  `remove_contact`, so a re-added peer's old messages can't be replayed in.
* **A sender told `Rejected(Replayed)`** for a DM concludes the peer saw
  higher seqs from it than it remembers (its state was rolled back, defence 1
  of message-protection.md's anti-rollback): it drops its Olm sessions with
  that peer (the next message starts a fresh pre-key session), jumps that
  stream forward by `SEQ_RESTORE_GAP` (2^20), renumbers its queued DMs and
  resends, once per message per flush.
* **Other streams** (server channels, Phase 1) use `check_sequence` with a
  stream name of their choice; `Fresh` is recorded. Server payloads are not
  checked by core in Phase 0, since their `seq` is per channel and the
  channel is inside the opaque data.
* **Olm** needs nothing more: a replayed Olm message fails to decrypt. Call
  signals carry no `seq`; `call_id` dedupe in `calls/` covers the rest.

### Restore from an old copy (Phase 0, [message-protection.md](message-protection.md#anti-rollback))

```rust
fn epoch(&self) -> Result<String>;                                  // "<id>.<n>"
fn check_epoch(&self, witness: Option<&str>) -> Result<EpochStatus>;
fn recover_from_restore(&self) -> Result<String>;                   // the "restore flag"
fn last_restore_ms(&self) -> Result<Option<i64>>;
struct EpochStatus { epoch: String, restored: bool }
```

Keys are never reused after a restore. The database carries an **epoch**
`(id, n)` (`meta.epoch`). `n` goes up before every Olm encrypt and before
every one-time key is handed out, and the same value is written atomically
to a sidecar file `<data_dir>/epoch` outside SQLCipher (database first, so a
crash in between never looks like a restore). A restore is detected when a
witness is ahead of the database (same id with a larger `n`, or an id this
database has never had):

* **at open**, from the sidecar. A database that has an epoch but no sidecar
  is treated as restored too, so the app should exclude `epoch` from backups
  and device transfers (Android `dataExtractionRules`; backups are meant to
  be off anyway);
* **from the app**, with `check_epoch(Some(copy))`, where `copy` is the
  epoch the app keeps in the OS keystore item next to `db_key` (from an
  earlier `check_epoch`). Call it after `init` and store the returned
  `epoch` back. This also catches a restore of the whole data directory, as
  long as the keystore wasn't rolled back too.

On detection (or an explicit `recover_from_restore()`), core:

1. drops **every Olm session**, so the next message to each peer starts a
   fresh pre-key session;
2. forgets the published **one-time key** pool (keys from the old copy may
   have been handed out since) and **rotates the fallback key**; the next
   key claim publishes a fresh batch;
3. **jumps every outgoing sequence number** forward by `SEQ_RESTORE_GAP`
   (2^20), including a floor for streams it doesn't know yet, and renumbers
   queued DMs;
4. records a **new epoch** (new id, the old one kept in `meta.epoch_history`
   so an old keystore copy doesn't trigger again) and `meta.restored_at`.

A rollback that no witness catches (the data dir *and* keystore restored) is
still caught by peers: they reject our reused `seq` as `Replayed`, and we
re-key and jump for that peer (see *Anti-replay*). Not in Phase 0: Megolm
outbound rotation (no Megolm yet), the `Heads` exchange after `Hello`
(Phase 1), bumping the epoch on expiry runs, and the FFI / UI side (these
calls are engine-level; the app can't call `check_epoch` or show "This
device's data was restored from an older copy" until they're exposed).

## Servers, Phase 1: model, roles, channels and invites (FFI)

The server model from [servers.md](servers.md) ("Membership, roles and
moderation", "Invites and joining"). Exposed over the FFI (`api/umbra.rs`);
all calls are `async`, and in mock mode they return an error ("servers are
not available in mock mode yet"). Per-sender Megolm, channel messages,
files, host audits and expiry come in later PRs. Until then servers have a
roster, roles, channels and invites, but no messages.

```rust
// Servers
async fn create_server(name: String, recovery_keys: Vec<String>,
                       display_name: Option<String>) -> Result<CreatedServer>;
async fn list_servers() -> Result<Vec<ServerInfo>>;
async fn server_info(server_id: String) -> Result<ServerInfo>;
async fn leave_server(server_id: String) -> Result<()>;
// Channels (text only)
async fn list_channels(server_id: String) -> Result<Vec<ServerChannel>>;
async fn create_channel(server_id: String, name: String) -> Result<ServerChannel>;   // owner, admins
async fn rename_channel(server_id: String, channel_id: String, name: String) -> Result<()>;
// Roles, policy, moderation
async fn list_members(server_id: String) -> Result<Vec<ServerMember>>;
async fn set_role(server_id: String, peer_id: String, role: ServerRole) -> Result<()>; // owner
async fn set_invite_roles(server_id: String, roles: Vec<ServerRole>) -> Result<()>;   // owner
async fn set_history_shared(server_id: String, shared: bool) -> Result<()>;           // owner
async fn set_server_display_name(server_id: String, display_name: Option<String>) -> Result<()>;
async fn kick(server_id: String, peer_id: String) -> Result<()>;   // owner; admins on members
async fn ban(server_id: String, peer_id: String) -> Result<()>;
async fn unban(server_id: String, peer_id: String) -> Result<()>;
// Invites and joining
async fn create_invite(server_id: String, opts: InviteOptions) -> Result<CreatedInvite>;
async fn revoke_invite(invite_id: String) -> Result<()>;
async fn parse_invite(link: String) -> Result<InvitePreview>;
async fn join_server(link: String, display_name: Option<String>) -> Result<ServerInfo>;
async fn join_requests(server_id: String) -> Result<Vec<JoinRequest>>;
async fn approve_join(server_id: String, peer_id: String, approve: bool) -> Result<()>;
```

Events arrive as `CoreEvent::Server { server_id, event: ServerEvent }`, with
`ServerEvent` one of `Joined`, `Updated { seq }`, `MemberJoined { peer_id }`,
`MemberLeft { peer_id, reason }`, `JoinRequest { request }`,
`ManifestRollback { have, got }`, `Forked { seq }` and `Removed { reason }`
(`MemberLeftReason`: `Left | Kicked | Banned`). The app maps them to one
`ServerActivity` event for now; there is no server UI yet.

**Defaults (Nyx).**
* **History**: new members see prior history by default. This is a
  per-server setting, `policy.history` in the manifest (`ServerInfo.
  history_shared`, default `true`). Only the owner changes it, with
  `set_history_shared`. Each member's local `server_memberships.
  history_visibility` follows the manifest. Megolm (PR 2) enforces it.
* **Identity**: one default identity (the device's PeerId and Olm keys) in
  every server. `display_name` on `create_server` / `join_server` and
  `set_server_display_name` set the name shown in that server only
  (`ServerInfo.my_display_name`; `None` = the global name). The card v2
  signature covers only the PeerId and identity key, so the name can differ
  per server without a new identity. Separate per-server identities
  (`ServerIdentity::Dedicated`) stay optional and are not implemented yet.
  On the owner device, a name change rewrites the roster entry at once.
  Other members' new names apply from their next join, because PR 1 has no
  profile op.
* **Invites**: authorized only. The roles in `policy.invite` may create
  invites (default owner and admin; the owner always may). An invite lasts
  7 days and has 1 use by default (at most 1000 uses), optionally with
  approval.

**Model** (`engine/server_model.rs`).
* **`ServerManifest`** (CBOR, signed with the owner key `K_S` under the
  context `umbra-manifest-v1`). Fields: `server_id` (`None` only at genesis),
  `seq`, `prev` (SHA-256 of the previous signed bytes), `owner`, `recovery`
  (1 to 3 keys, required, none equal to `owner`), `name`, `icon`, `channels`,
  `roster` (v2 cards **without addresses**, plus `role`, `added_by` and
  `added_seq`), `bans`, `hosts`, `packs`, `policy` and `takeover`.
  * The **ServerId** is the base64url SHA-256 of the signed genesis bytes.
  * A **ChannelId** is 16 random bytes in base64url.
  * Manifests are at most 256 KiB, and so are `ServerManifest` payloads.
    Other kinds stay at 64 KiB (`servers::max_server_payload`).
* **`K_S`** is generated on `create_server`, kept only in the `servers`
  table, and never crosses the FFI. When no recovery keys are given, one is
  generated and its secret is returned once in `CreatedServer.
  recovery_secret`; core never stores it.
* **Accepting a manifest** (`check_next`):
  * `seq + 1` needs `prev` to match and a signature by the current `owner`,
    or by one of the current `recovery` keys when `takeover` is set.
  * A gap is accepted only if the same owner key signs it.
  * A lower seq is a **rollback** (S5). It is refused and raises
    `ManifestRollback`.
  * The same seq with different bytes, validly signed, is a **fork**. It is
    stored in `manifest_forks`, the server is **frozen** read-only
    (`ServerStatus::Frozen`), and `Forked` is raised.
  * A joiner's first manifest is anchored on the owner key named in the
    invite (`check_first`). The invite preview shows that key's
    fingerprint.
* **`RosterOp`** (`Admit | Kick | Ban | Unban | DeleteMessage |
  CreateChannel | EditChannel`). Each op carries `server_id`, `base_seq`,
  `ts`, the signer's PeerId and its Ed25519 key, and is signed with the
  signer's device key (context `umbra-roster-op-v1`). Rules:
  * The signer needs the permission in the manifest at `base_seq`, which
    must be the current manifest.
  * Admins act only on members, never on the owner or other admins.
  * `Admit` must carry an unexpired invite that the signer itself signed.
  * A **ban beats an admit**.
  * An **unban needs a newer `base_seq`** than the ban.
  * Ops apply in (`ts`, hash) order, so every member computes the same
    effective state.
* **Who changes what.** The **owner device writes manifests directly**, and
  only the owner changes roles, permissions, recovery keys and hosts. Admins
  send ops. The owner **folds** every accepted op into the next manifest
  straight away. An op on an older base (for example two admin ops before the
  fold) is rebased by the owner if it is at most 16 manifests old and the
  signer still holds the permission; unbans are never rebased. Ops older
  than the current manifest are pruned.
* **Distribution.** Manifests and ops go to every co-member through the
  outbox as `ServerManifest` / `ServerOp` payloads, and the engine consumes
  them (`subscribe_server` no longer sees those two kinds). Co-member
  records follow the effective roster, so pins, `KeyChanged { Roster }` and
  `server_ok` checks apply as in Phase 0. A removed member is no longer a
  co-member, so the outbox drops it; it gets the new manifest once, best
  effort and directly, then shows `Removed`.
* **Permissions** (`Permission`): `Invite` (roles in `policy.invite`), plus
  `ManageChannels`, `Kick`, `Ban` and `DeleteMessages` (owner and admins).
  Channels have read and post role lists (default: everyone), and `list_channels`
  reports `can_read` / `can_post` for us.

**Invites and joining.**
* **Link format**: `umbra://join?i=<base64url CBOR SignedInvite>&s=<base64url
  32-byte secret>`.
  * The signed part (context `umbra-invite-v1`, signed by the inviter's
    device key) holds `server_id`, `owner_key`, `server_name`, the inviter's
    v2 card (with addresses, so the joiner can dial), `invite_id`, the
    **SHA-256 of the secret**, `expires_at`, `max_uses`, `needs_approval` and
    `hosts`.
  * Only the hash is signed, so an `Admit` op can carry the invite to other
    members without the secret.
  * The inviter stores the secret with the invite in `server_invites`.
* **`JoinHello`**. The joiner sends `JoinHello { card, server_id, invite_id,
  proof }` on `/umbra/msg/1`, with `proof = HMAC-SHA256(secret,
  "umbra-join-v1" || joiner PeerId bytes)`. A proof can't be replayed by
  another peer. `JoinHello` has its own token bucket. The inviter answers
  one of:
  * `Ok`: admitted, or waiting for approval;
  * `Rejected(InviteInvalid)`: an unknown, expired, revoked or used-up
    invite, or a bad proof;
  * `Rejected(NotAuthorized)`: we may no longer invite, or the server is
    frozen, removed or full (250 members);
  * `Rejected(Banned)`.
* **Admitting.** The use count is taken atomically. Then:
  * The owner writes a manifest with the joiner in it.
  * An admin signs an `Admit` op and sends the joiner the current manifest
    and its ops.
  * The joiner shows `Joining` until the first manifest (and, for an admin
    invite, the `Admit` op) arrives, then raises `Joined`.
  * A refusal undoes the joiner's local records.
* **Approval invites** raise `JoinRequest` on the inviter. `approve_join`
  uses one invite use and admits the joiner.

**Store: schema v5** (`store_servers.rs`). The new tables are `servers`,
`manifests`, `manifest_forks`, `roster_ops`, `server_channels` and
`server_roles` (the effective state, materialized), plus `server_invites`,
`join_requests` and `pending_joins`.
* Every table is `CREATE TABLE IF NOT EXISTS`.
* The schema batch now runs **inside the migration transaction**, so a
  failed open (for example a newer `schema_version`) leaves no v5 tables
  behind, and re-running is a no-op.
* `leave_server` deletes all of a server's rows, its co-member records and
  its queued server payloads. It drops Olm sessions with peers we no longer
  share anything with.

**Not in PR 1:**
* Ownership transfer and recovery takeover from the API. The model checks
  them.
* A signed `Leave` op. Leaving is local, and the owner still lists us until
  it kicks us.
* Telling a joiner that a request was declined.
* Re-sending an admin op that the owner dropped because it was more than
  16 manifests stale.
* Address discovery for co-members beyond the inviter and the connection
  (roster cards carry no addresses; the DHT and hosts come later).
* `DeleteMessage` has no API until channel messages exist (PR 2).
* `hosts` grants (`umbra-host`, below) are not yet sent from core.

## Messaging

```rust
async fn send_text(peer_id: String, body: String) -> Result<MessageId>;
fn list_conversations() -> Result<Vec<ConversationSummary>>;
fn list_messages(peer_id: String, before: Option<i64>, limit: u32) -> Result<Vec<Message>>;
fn mark_read(peer_id: String, up_to: MessageId) -> Result<()>;
```

* `MessageId` = `String` (UUID v4).
* Max text body 64 KiB. Timestamps are the sender's clock.
* `send_text` persists the message and returns at once with status `Sent`.
  `Delivered` arrives when the peer acked; `Failed` only if it expires
  (7 days) or is rejected permanently.
* `list_messages` returns newest first; page with `before = oldest.timestamp_ms`.

## Calls signaling (for `calls/`)

```rust
async fn send_signal(peer_id: String, call_id: String, kind: SignalKind, payload: String) -> Result<()>;
```

Olm-encrypted, delivered directly, **not** stored or queued: returns an error
if the peer is unreachable right now (treat as "call failed"). Incoming
signals arrive as `CoreEvent::Signal`. `payload` is opaque to core (put SDP /
ICE JSON in it), max 64 KiB. All kinds, including `Accept` and `Busy`, take
the same path: never queued, same size cap.

## Types

```rust
struct CoreConfig {
    bootstrap_addrs: Vec<String>,   // multiaddrs with /p2p/<id>
    relay_addrs: Vec<String>,       // relay v2 servers with /p2p/<id>
    enable_mdns: bool,
    tor_socks: Option<String>,      // "127.0.0.1:9050" => Tor-only mode
    listen_addrs: Vec<String>,      // ADDITION, empty = defaults
    mock: bool,                     // ADDITION, in-memory mock backend
}
struct SelfInfo { peer_id, fingerprint, qr_payload, display_name, identity_key, ed25519_key, addrs: Vec<String> }
struct ContactCard { peer_id, identity_key, ed25519_key, addrs: Vec<String>, display_name: Option<String>,
                     sig: Option<String> /* card v2, serde(default) */ }
struct Contact { peer_id, display_name, identity_key, ed25519_key, addrs, verified: bool,
                 safety_number, added_at_ms: i64, online: bool,
                 key_changed: bool, pending_identity_key: Option<String>, signed_card: bool }
enum KeyChangeSource { Hello, KeyClaim, ContactCard, Roster }
struct Message { id, peer_id, outgoing: bool, body, timestamp_ms: i64, status: DeliveryStatus, read: bool }
struct ConversationSummary { peer_id, display_name, last_message: Option<Message>, unread_count: u32, updated_at_ms: i64 }
struct NetworkInfo { running: bool, listen_addrs: Vec<String>, connected_peers: u32, relay_reservations: u32 }
enum DeliveryStatus { Sent, Delivered, Failed, Received }
enum SignalKind { Offer, Answer, IceCandidate, Hangup, Ringing, Reject, Accept, Busy }
// Wire (JSON in the Olm payload) = variant name: "Offer", "Answer",
// "IceCandidate", "Hangup", "Ringing", "Reject", "Accept", "Busy";
// must match calls/lib/src/signaling.dart. Flow (calls/docs/signaling.md):
// Ringing -> Accept | Busy | Reject -> Offer -> Answer -> IceCandidate* -> Hangup
enum CoreEvent {
    MessageReceived { message: Message },
    MessageStatus { id: MessageId, peer_id: String, status: DeliveryStatus },
    ContactOnline { peer_id: String },
    ContactOffline { peer_id: String },
    ContactRequest { card: ContactCard },
    Signal { peer_id: String, call_id: String, kind: SignalKind, payload: String },
    NetworkStatus { info: NetworkInfo },
    KeyChanged { peer_id: String, old_identity_key: String, new_identity_key: String,
                 old_safety_number: String, new_safety_number: String, source: KeyChangeSource },
}
```

Errors are `anyhow::Result` (Dart: `AnyhowException` with a message).

## Deviations from the v0 sketch

1. Read functions (`self_info`, `list_contacts`, `list_conversations`,
   `list_messages`) return `Result<..>` instead of a bare value, so
   "not initialized" and DB errors surface instead of being swallowed.
   `set_verified`, `remove_contact`, `mark_read` return `Result<()>`.
2. `CoreEvent` variants all use named fields (`ContactRequest { card }`,
   `NetworkStatus { info }`, `MessageReceived { message }`).
   `MessageStatus.status` is `DeliveryStatus`, which adds `Received` for
   incoming messages; `MessageStatus` events also carry `peer_id`.
3. Additions: `CoreConfig.listen_addrs`, `CoreConfig.mock`, `network_status()`,
   `SelfInfo.{identity_key, ed25519_key, addrs}`, optional `name=` in the QR,
   `Contact.{safety_number, added_at_ms, online, key_changed, pending_identity_key}`,
   `CoreEvent::KeyChanged`, `accept_key_change`, `reject_key_change`,
   `ContactCard.sig` / QR `sig=` (card v2), `Contact.signed_card`.

## Protocol summary

* **Identity**: Ed25519 device key (libp2p) → PeerId + Noise auth; vodozemac
  Olm account → Curve25519 identity key. Both stored in SQLCipher; the Olm
  pickles are additionally encrypted with `HKDF-SHA256(db_key, "olm-pickle-v1")`.
* **Safety number**: Signal's numeric-fingerprint construction (5200 ×
  SHA-512 over `ed25519 || curve25519` + PeerId), 30 digits per side,
  sorted and concatenated.
* **`/umbra/keys/1`** (CBOR): `ClaimOneTimeKey` → `Key { identity_key,
  one_time_key }`. Served **only to contacts**, rate-limited, falling back to
  the fallback key when one-time keys are rationed (see *Abuse limits*).
  The claimer checks `identity_key` equals the one from the QR.
* **`/umbra/msg/1`** (CBOR): `Hello { card }` (once per connection) or
  `Olm { t, c }`. The response is the ack: `Ok`, `Delivered`, or
  `Rejected(NotAContact | DecryptFailed | Malformed | Internal | KeyMismatch |
  RateLimited | NotAMember | Replayed | InviteInvalid | NotAuthorized |
  Banned)`. `JoinHello { card, server_id, invite_id, proof }` asks to join a
  server (see *Servers, Phase 1*). (Older cores can't decode
  `Replayed`; only new cores send it, and only for payloads carrying `seq`,
  which only new cores send.)
  Olm plaintext is JSON `{v, to, id, ts, body: Text | Signal | Server,
  seq?, prev?, expires_at?}`; `to` must be
  the receiver's PeerId. Olm v1 (libolm-compatible) sessions, up to 4 per
  peer, pre-key messages matched by session id.
* **Padding** ([message-protection.md](message-protection.md#padding)):
  every Olm plaintext core sends is padded, before encryption, to the next
  bucket of **256 B, 1 KiB, 4 KiB, 16 KiB or 64 KiB**; a plaintext over 64 KiB
  (a full 64 KiB text or signal plus JSON framing and escaping) rounds up to a
  multiple of 64 KiB, well under the 1 MiB codec cap. The padding is trailing
  ASCII spaces after the JSON, which every JSON parser skips, so it is
  backward-compatible both ways: older cores parse padded payloads, and new
  cores accept unpadded ones. The receiver strips trailing whitespace before
  parsing. The usable caps don't change (text 64 KiB, signal payload 64 KiB,
  server data 64 KiB, manifests 256 KiB). Padding hides lengths within a bucket, not message
  counts or timing. (`proto::{PAD_BUCKETS, padded_len, encode_payload,
  decode_payload}`.)
* **Server payload carrier (Phase 0)**: `PayloadBody::Server { server_id,
  kind: ServerPayloadKind, data }`, with `kind` one of `ServerKey |
  ServerKeyForward | ServerKeyRequest | ServerManifest | ServerOp` and `data`
  opaque unpadded base64, at most 64 KiB decoded. **This deviates from
  servers.md**, which lists those as separate top-level `PayloadBody`
  variants. One carrier keeps `server_id` outside the opaque bytes, so the
  co-member check ("is the sender in *this* server?") is one uniform check
  even for `ServerManifest { bytes }` / `ServerOp { bytes }`. Phase 1 defines
  each kind's `data`. `ServerInvite` is a DM and is not a server payload.
  Older cores reply `Rejected(Malformed)` (accepted before 1.0, as in
  servers.md).
* **Contact flow**: Alice scans Bob → Alice's Hello reaches Bob → Bob gets
  `ContactRequest` (at most once per identity per 10 min until accepted,
  subject to the limits below; still dedupe by `peer_id` in the UI). Messages Alice sends meanwhile wait in her outbox and flush
  when Bob calls `add_contact`.
* **Outbox**: retried on connect, on Hello, on mDNS/DHT discovery and every
  15 s; duplicates are dropped by message id; `DecryptFailed` resets the
  session once and retries.
* **Peer discovery**: QR addrs, mDNS (if enabled), identify, Kademlia
  (`/umbra/kad/1.0.0`, own DHT — needs `bootstrap_addrs`), relay v2
  circuits + DCUtR hole punching (`relay_addrs`).

## Abuse limits

Inbound traffic from strangers and key claims are throttled in
`core/src/engine/limits.rs`. Each value is a `pub const` with the default
below; `Core::open_with_limits(.., Limits { .. })` overrides them (used by
tests; the bridge uses the defaults).

| Constant | Default | Effect |
|---|---|---|
| `HELLO_BURST` / `HELLO_REFILL` | 5 / 1 per 10 s | Per-peer token bucket on `Hello` (contacts too). Excess gets `Rejected(RateLimited)`. |
| `JOIN_HELLO_BURST` / `JOIN_HELLO_REFILL` | 3 / 1 per 20 s | Per-peer bucket on `JoinHello`, separate from `Hello`. |
| `CONTACT_REQUEST_WINDOW` | 10 min | At most one `ContactRequest` event per unknown identity per window. |
| `CONTACT_REQUEST_BURST` / `_REFILL` | 10 / 1 per 10 s | Global budget of `ContactRequest` events (sybil floods). |
| `MAX_PENDING_CONTACT_REQUESTS` | 64 | Distinct unanswered requesters tracked; beyond that new requests are dropped (logged). `add_contact` frees a slot. |
| `PENDING_CONTACT_REQUEST_TTL` | 24 h | A pending request is forgotten after this. |
| `OTK_CLAIM_BURST` / `_REFILL` | 3 / 1 per 60 s | Per-contact `ClaimOneTimeKey` bucket; excess gets `Refused(RateLimited)` (the claimer retries later). |
| `MAX_OUTSTANDING_OTKS_PER_PEER` | 2 | Fresh one-time keys a contact may hold unconsumed. Beyond that it gets the **fallback key**, so its session still works but it can't drain the pool. |
| `MAX_OUTSTANDING_OTKS_TOTAL` | 50 | Same, across all contacts. |
| `OUTSTANDING_OTK_TTL` | 1 h | An unconsumed key stops counting after this. |
| `OTK_POOL_LOW_WATER` / `OTK_POOL_BATCH` | 5 / 20 | The published pool is topped up by 20 keys when fewer than 5 remain. |
| `MAX_TRACKED_PEERS` | 1024 | Bound on per-peer bucket state (idle buckets, then least recently used, are evicted). |

Notes:

* A key claim never blocks: a contact gets a fresh key, the fallback key, or
  `RateLimited`. One contact's flood doesn't affect other contacts' claims
  or sessions.
* A one-time key stops counting as outstanding once a pre-key message using
  it is decrypted.
* The fallback key is generated once and not rotated; its public half is kept
  in the store (`olm_fallback_key`) because vodozemac only reports it while
  unpublished.
* Limiter state lives in memory and resets on restart. The OTK pool is
  persisted.

## Tor mode

Tor-only when `CoreConfig.tor_socks = "ip:port"` (system tor / Orbot /
`arti proxy`), or when built with cargo feature `tor` (embedded arti via
`umbra-tor`; see `docs/tor.md`):

* base transport is the Tor dialer **only** (never `or()`-ed with TCP/QUIC);
  no local DNS, no listening, no mDNS, no DCUtR, Kademlia client mode,
  identify hides listen addresses;
* inbound reachability only via `relay_addrs` circuits (QR advertises only
  `/p2p-circuit` addresses);
* calls (`calls/`, WebRTC) are not routed through Tor — the UI must say so.

Features: `tor` (embedded arti, Tor-only) and `tor-listen` (**EXPERIMENTAL**,
implies `tor`: compiles umbra-tor's onion-service hosting and sets the onion
nickname `"umbra"`; core does **not** call `listen_on` yet). Keep both out of
release builds until the arti APK delta is measured.

Embedded arti state (`tor` feature):

* Built with `TorTransport::bootstrapped_with(TorConfig::new().state_dir(..)
  .cache_dir(..)[.onion_nickname("umbra")])`, never arti's platform defaults
  (those don't exist on Android).
* Dirs: `<data_dir>/tor/state` and `<data_dir>/tor/cache`, created by
  `start_network` and chmod'ed to **0700** on Unix (arti's fs-mistrust
  refuses group/world-accessible dirs; Android app-private dirs already are).
* `<data_dir>/tor/state` holds guard state and, with `tor-listen`, the onion
  service identity keystore (`state/keystore/`). **Wiping it permanently
  changes the `.onion` address**, so back it up and restore it together with
  `umbra.db` (the Olm/identity store). The nickname is the keystore namespace;
  changing it also changes the address.
* arti 0.35 quirk: an onion service dropped **before bootstrap completed**
  keeps its state lock until the process exits, and relaunching the same
  nickname in that process fails with `AlreadyLocked`. Core doesn't host
  onion services yet; once it does, don't stop and restart networking
  (`stop_network` / `start_network`) before Tor has bootstrapped. Restart
  the process instead.

## Mock backend (`mock: true`)

* `start_network` emits a running `NetworkStatus`.
* ~2 s after `init` a `ContactRequest` from "Mock Bot" arrives.
* `add_contact` -> `ContactOnline` after 300 ms.
* `send_text` -> `Sent`, `Delivered` (~400 ms), then an `echo: <text>` reply (~1 s).
* `send_signal(Ringing)` -> `Accept`; `send_signal(Offer)` -> `Answer` with
  payload `mock-answer-sdp`; `Hangup` is echoed back; other kinds are ignored.
