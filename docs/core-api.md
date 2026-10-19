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
umbra://add?peer=<PeerId>&ik=<curve25519 b64>&ed=<ed25519 b64>&addr=<ma1>,<ma2>[&name=<display name>]
```

`parse_qr` checks that `ed` derives `peer` and that `ik` is a valid
Curve25519 key. Base64 is standard alphabet, unpadded (URL-encoded in the
query). At most 6 addresses are written.

* `SelfInfo.fingerprint`: our 30-digit half (6 groups of 5).
* `Contact.safety_number`: 60 digits (12 groups of 5), identical on both
  devices. Show it on the contact screen and let the user mark `verified`.

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
ICE JSON in it), max 64 KiB.

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
struct ContactCard { peer_id, identity_key, ed25519_key, addrs: Vec<String>, display_name: Option<String> }
struct Contact { peer_id, display_name, identity_key, ed25519_key, addrs, verified: bool,
                 safety_number, added_at_ms: i64, online: bool,
                 key_changed: bool, pending_identity_key: Option<String> }
enum KeyChangeSource { Hello, KeyClaim, ContactCard }
struct Message { id, peer_id, outgoing: bool, body, timestamp_ms: i64, status: DeliveryStatus, read: bool }
struct ConversationSummary { peer_id, display_name, last_message: Option<Message>, unread_count: u32, updated_at_ms: i64 }
struct NetworkInfo { running: bool, listen_addrs: Vec<String>, connected_peers: u32, relay_reservations: u32 }
enum DeliveryStatus { Sent, Delivered, Failed, Received }
enum SignalKind { Offer, Answer, IceCandidate, Hangup, Ringing, Reject }
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
   `CoreEvent::KeyChanged`, `accept_key_change`, `reject_key_change`.

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
  RateLimited)`.
  Olm plaintext is JSON `{v, to, id, ts, body: Text | Signal}`; `to` must be
  the receiver's PeerId. Olm v1 (libolm-compatible) sessions, up to 4 per
  peer, pre-key messages matched by session id.
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
* `send_signal(Offer)` -> `Ringing`, then `Answer` with payload `mock-answer-sdp`;
  `Hangup` is echoed back.
