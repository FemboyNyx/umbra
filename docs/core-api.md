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

`core/src/frb_generated.rs` is a **placeholder** so the crate compiles before
codegen; `flutter_rust_bridge_codegen generate` overwrites it.

## Status

API is stable at v0 for app/ and calls/ to code against. Pass
`CoreConfig { mock: true }` to get the in-memory mock backend (no disk, no
network) while the real backend lands.

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

## Messaging

```rust
async fn send_text(peer_id: String, body: String) -> Result<MessageId>;
fn list_conversations() -> Result<Vec<ConversationSummary>>;
fn list_messages(peer_id: String, before: Option<i64>, limit: u32) -> Result<Vec<Message>>;
fn mark_read(peer_id: String, up_to: MessageId) -> Result<()>;
```

* `MessageId` = `String` (UUID v4).
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
                 safety_number, added_at_ms: i64, online: bool }
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
   `Contact.{safety_number, added_at_ms, online}`.

## Mock backend (`mock: true`)

* `start_network` emits a running `NetworkStatus`.
* ~2 s after `init` a `ContactRequest` from "Mock Bot" arrives.
* `add_contact` -> `ContactOnline` after 300 ms.
* `send_text` -> `Sent`, `Delivered` (~400 ms), then an `echo: <text>` reply (~1 s).
* `send_signal(Offer)` -> `Ringing`, then `Answer` with payload `mock-answer-sdp`;
  `Hangup` is echoed back.
