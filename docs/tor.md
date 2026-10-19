# Optional Tor routing (spike)

Status: **experimental spike** in `core/tor/` (`umbra-tor`). Ready for Elite to
uncomment the workspace member. AGPL-3.0 with the rest of Umbra.

## Goal

Give Umbra an opt-in path that hides **where dials go** (IP / ASN of the
destination, and the fact that two peers are talking) behind the Tor network,
without baking Tor into every build.

This does **not** replace vodozemac E2E encryption. Tor is metadata protection
for the transport; message contents stay E2E regardless.

## Approaches evaluated

### 1. `libp2p-community-tor` (rejected for now)

Crate: [`libp2p-community-tor`](https://crates.io/crates/libp2p-community-tor) 0.4.1
([umgefahren/libp2p-tor](https://github.com/umgefahren/libp2p-tor)).

| | |
| --- | --- |
| Pros | Ready-made `Transport`, dials via arti |
| Cons | Pins `arti-client ^0.24` and `libp2p ^0.53` (stale vs Umbra's stack); last release 2024-11; dial-only in practice |
| Verdict | **Do not depend on it.** Re-check yearly; if it tracks current arti + libp2p, prefer it over maintaining our own wrapper. |

### 2. Embedded arti (`--features tor`) — implemented

`umbra-tor` optionally depends on **arti-client 0.35** (rusqlite 0.37, same as
`umbra_core`) and exposes `TorTransport`: a dial-only `libp2p::core::Transport`.

```text
app  →  Elite transport builder  →  TorTransport (arti)  →  Tor network  →  peer
```

Pinned with Elite:

- `libp2p` **0.55** (workspace pin)
- `arti-client` **0.35** / rusqlite **0.37** (umbra-tor enables arti `static-sqlite` so CI/Android need no system libsqlite3)
- MSRV **1.89** (vodozemac 0.11)

#### Custom state / cache dirs (Android)

Arti's defaults use XDG-style paths that do not exist on Android. Pass app-owned
directories via `TorConfig`:

```rust
use umbra_tor::{TorConfig, TorTransport};

let cfg = TorConfig::new()
    .state_dir(files_dir.join("arti-state"))
    .cache_dir(cache_dir.join("arti-cache"));
let tor = TorTransport::bootstrapped_with(cfg).await?;
```

Under the hood this calls `TorClientConfigBuilder::from_directories` (literal
`CfgPath`s). Both dirs must be set, or neither (platform defaults).

### 3. External SOCKS5 fallback — designed (+ `/onion3` mapping)

When embedding arti is too heavy, point a dial-only SOCKS transport at:

- system `tor` `SocksPort` → `127.0.0.1:9050`
- `arti proxy` (often `9150`)
- Orbot on Android

Helpers:

- `multiaddr_to_socks_dest` / `target_to_socks_dest`
- `/onion3/<56chars>:<port>` → SOCKS5 **domain** target `<56chars>.onion:port`
  (ATYP domain — remote resolution, no local DNS)

Prefer [`libp2p-socks5`](https://crates.io/crates/libp2p-socks5) for the actual
transport. Use SOCKS **instead of** clearnet TCP on that swarm, not beside it.

## What works / what does not

| Capability | Status |
| --- | --- |
| Dial clearnet TCP over Tor exits | Yes (embedded arti or SOCKS) |
| Dial `/dns[4|6]/…/tcp/…` without local DNS | Yes |
| Dial `/onion3/…` (v3) via arti or SOCKS domain | Yes (`onion-service-client` under `tor`; SOCKS maps to `.onion`) |
| Custom arti state/cache dirs | Yes (`TorConfig`) |
| Listen / host an onion service | **Not yet.** `listen_on` → `MultiaddrNotSupported` |
| UDP / QUIC over Tor | **No.** Tor is TCP-only |
| WebRTC (`calls/`) over Tor | **No.** Keep A/V clearnet (still E2E DTLS-SRTP) |
| Mobile size / battery | Prefer SOCKS + Orbot on Android until arti APK delta is measured |

## Privacy misuse warnings

- A Tor transport does **not** make a normal libp2p node private by itself.
- `identify`, listen addrs, and any clearnet transport on the same swarm can
  correlate Tor circuits with a real IP.
- Prefer Tor-only dials and no clearnet listen addrs when “Tor mode” is on.

## Enable steps (Elite `core/`)

`core/tor/` already lives in-tree. In `core/Cargo.toml`:

1. Uncomment the workspace member `"tor"`.
2. Uncomment `umbra-tor = { path = "tor", optional = true, features = ["tor"] }`.
3. Uncomment / add feature `tor = ["dep:umbra-tor"]`.
4. Branch the transport builder on `TorMode`; for Android embedded mode call
   `TorTransport::bootstrapped_with(TorConfig::…)`.
5. Do **not** `or()` Tor with clearnet TCP.
6. Keep `calls/` WebRTC on its own path; expose `torAvailable` / `torMode` to
   Copilot's `app/` so the setting can hide itself when the feature is off.

## How to build / test this crate

```bash
export PATH="$HOME/.cargo/bin:$PATH"   # rustup stable >= 1.89
cd core/tor
cargo fmt
cargo test
cargo test --features tor
cargo clippy --all-targets -- -D warnings
cargo clippy --all-targets --features tor -- -D warnings
cargo run --example dial_info -- /onion3/2gzyxa5ihm7nsggfxnu52rck2vv4rvmdlkiu3zzui5du4xyclen53wid:80
```

No secrets belong here. arti state/cache dirs are local to the device/user.

## Open questions

- Product: Tor for messaging only until onion **listen** exists?
- Android: measure arti APK delta vs Orbot SOCKS.
- Gem (`calls/`): UX must say calls leave Tor when Tor mode is on.
