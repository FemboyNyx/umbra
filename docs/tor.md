# Optional Tor routing (spike)

Status: **experimental spike** in `core/tor/` (`umbra-tor`). Ready for Elite to
uncomment the workspace member. Inbound onion listening is a separate,
**EXPERIMENTAL**, off-by-default feature (`tor-listen`). AGPL-3.0 with the rest of Umbra.

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
- MSRV from `core/rust-toolchain.toml` (currently **1.99.1**, vodozemac 0.11)

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
| Listen / host an onion service | **EXPERIMENTAL**, feature `tor-listen` (off by default). Without it `listen_on` → `MultiaddrNotSupported`. See below. |
| UDP / QUIC over Tor | **No.** Tor is TCP-only |
| WebRTC (`calls/`) over Tor | **No.** Keep A/V clearnet (still E2E DTLS-SRTP) |
| Mobile size / battery | Prefer SOCKS + Orbot on Android until arti APK delta is measured |

### 4. Inbound onion-service listening (`--features tor-listen`) — EXPERIMENTAL

> ⚠️ **Experimental.** arti's onion-service *hosting* (`onion-service-service`,
> `tor-hsservice`) is itself experimental upstream. Off by default; do not
> enable in release builds or promise it in UI until it has soaked.

`tor-listen` implies `tor` and enables arti's `onion-service-service`. With it,
`TorTransport::listen_on` accepts `/onion3` addresses and hosts a v3 onion
service via `TorClient::launch_onion_service` + `tor_hsservice::handle_rend_requests`.
Incoming `BEGIN` streams on the virtual port are accepted (`CONNECTED`) and
surfaced as `TransportEvent::Incoming`; anything else gets `END(DONE)` (same as
other onion implementations, so we are not distinguishable).

**Listen address convention** (a node can't know its `.onion` before the key exists):

| `listen_on` addr | Meaning |
| --- | --- |
| `umbra_tor::onion_listen_addr(port)` (all-zero `/onion3` payload) | Wildcard: host *my* service on virtual `port` |
| `/onion3/<own 56 chars>:<port>` | Same, but refuse (`OnionAddressMismatch`) unless the persisted identity is exactly this |
| anything else (TCP, QUIC, `/onion3/...:port/p2p/...`, port 0) | `MultiaddrNotSupported` |

The real address is reported via `TransportEvent::NewAddress`
(`/onion3/<own>:<port>`). Because onion clients are anonymous, `send_back_addr`
on incoming connections is our own onion address, not the remote's.

```rust
use umbra_tor::{onion_listen_addr, TorConfig, TorTransport};

let cfg = TorConfig::new()
    .state_dir(files_dir.join("arti-state"))   // must be private (0700)
    .cache_dir(cache_dir.join("arti-cache"))
    .onion_nickname("umbra");                   // keystore namespace; keep stable
let tor = TorTransport::bootstrapped_with(cfg).await?;
// …build the swarm with `tor` as the transport, then:
swarm.listen_on(onion_listen_addr(4001))?;
```

#### Key storage

- The onion identity key (`KS_hs_id`) and other service keys are generated by
  arti on first launch and stored **only** in arti's native keystore at
  `<state_dir>/keystore/` (plus service state under `<state_dir>`), keyed by
  the **nickname**. Same state dir + nickname ⇒ same `.onion` across restarts.
- Listening is **refused** (`ListenRequiresCustomDirs`) unless the transport was
  built from a `TorConfig` with explicit `state_dir` + `cache_dir`
  (`bootstrapped_with` / `unbootstrapped_with`), so keys never land in a
  platform default location. `from_client` / `with_config` transports can't listen.
- arti's fs-mistrust refuses group/world-accessible state dirs: create them
  `0700` (Android app-private dirs already are).
- umbra-tor never reads, copies, or logs key material. It logs only nickname
  and port at `info` — not even the (public) onion address. arti's own logs use
  `safelog` redaction for addresses.
- Backup/wipe: deleting `<state_dir>/keystore` permanently loses the `.onion`
  identity; treat it like the Olm identity store.
- Nickname: arti `HsNickname`/slug rules — lowercase ASCII letters, digits,
  `_`, `-`; non-empty, must not start with `-`; on Windows also not `con`,
  `nul`, `com1`, … Invalid → `InvalidNickname`. Default `"umbra"`.

#### Limitations

- **arti HS maturity:** experimental upstream; API and on-disk formats may change
  between arti releases (we pin 0.35). Expect fixes to require bumping arti,
  rusqlite and libp2p together.
- **No client authorization** (restricted discovery) yet: anyone who learns the
  `.onion` can reach the service and attempt a libp2p handshake. Noise still
  authenticates peers, but reachability is public.
- **DoS:** no proof-of-work (`hs-pow-full` not enabled) or intro rate limiting
  configured; an attacker who knows the address can flood intro/rend circuits
  and burn CPU/battery. Defaults only (`max_concurrent_streams_per_circuit`, …).
- **Reachability latency:** after launch the service must build intro circuits
  and publish descriptors; first reachability typically takes tens of seconds to
  minutes (longer on mobile networks). Dialers may fail until then.
- **One listener per transport** (`OnionListenerActive` otherwise); one
  virtual port per listener. arti locks per-nickname state while the service runs.
- **In-process relaunch:** in arti 0.35 the descriptor publisher waits for a
  network directory without watching for shutdown, so if a service is dropped
  **before bootstrap completed**, its state lock isn't released until the
  process exits and relaunching the same nickname fails with `AlreadyLocked`.
  Relaunch after a normal bootstrapped run, or across process restarts, is fine.
- **Mobile background:** Android may kill/suspend the process; the service is
  only reachable while the app runs (needs a foreground service to stay up).
- Still TCP-only: QUIC and WebRTC calls do not go over the onion service.

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
5. Optional, **EXPERIMENTAL**: forward a `tor-listen` feature
   (`tor-listen = ["tor", "umbra-tor/tor-listen"]`) and call
   `swarm.listen_on(umbra_tor::onion_listen_addr(port))`; keep it out of
   default/release features.
6. Do **not** `or()` Tor with clearnet TCP.
7. Keep `calls/` WebRTC on its own path; expose `torAvailable` / `torMode` to
   Copilot's `app/` so the setting can hide itself when the feature is off.

## How to build / test this crate

```bash
export PATH="$HOME/.cargo/bin:$PATH"   # rustup toolchain matching `core/rust-toolchain.toml`
cd core/tor
cargo fmt
cargo test
cargo test --features tor
cargo test --features tor-listen        # EXPERIMENTAL; offline (no bootstrap)
cargo clippy --all-targets -- -D warnings
cargo clippy --all-targets --features tor -- -D warnings
cargo clippy --all-targets --features tor-listen -- -D warnings
cargo run --example dial_info -- /onion3/2gzyxa5ihm7nsggfxnu52rck2vv4rvmdlkiu3zzui5du4xyclen53wid:80
```

No secrets belong here. arti state/cache dirs are local to the device/user.

## Open questions

- Product: ship onion **listen** (`tor-listen`) only after it soaks; until then
  Tor mode is dial-only (reach peers via relay/rendezvous).
- Client authorization + PoW for the onion service before any non-experimental use.
- Android: measure arti APK delta vs Orbot SOCKS.
- Gem (`calls/`): UX must say calls leave Tor when Tor mode is on.
