# Optional Tor routing (spike)

Status: **experimental spike** in `core/tor/` (`umbra-tor`). Not wired into
Elite's swarm yet. AGPL-3.0 with the rest of Umbra.

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
| Cons | Pins `arti-client ^0.24` and `libp2p ^0.53` (stale vs Umbra's stack); last release 2024-11; ~7k downloads; dial-only in practice |
| Verdict | **Do not depend on it.** Re-check yearly; if it tracks current arti + libp2p, prefer it over maintaining our own wrapper. |

### 2. Embedded arti (`--features tor`) — implemented

`umbra-tor` optionally depends on **arti-client 0.35** (MSRV 1.85.1, matches the
dev box) and exposes `TorTransport`: a dial-only `libp2p::core::Transport` that
bootstraps an in-process Tor client and dials TCP / DNS / onion3 multiaddrs.

```text
app  →  Elite transport builder  →  TorTransport (arti)  →  Tor network  →  peer
```

Pinned on purpose:

- `libp2p = "=0.55.0"` — 0.56+ needs rustc ≥ 1.88; this environment has 1.85.1.
- `arti-client = "=0.35.0"` — newer arti bumps MSRV past 1.85.

Elite should align the workspace `libp2p` version with this pin, or bump both
together when the CI/Android NDK rustc is newer.

### 3. External SOCKS5 fallback — designed

When embedding arti is too heavy (APK size, memory, battery), point a dial-only
SOCKS transport at:

- system `tor` `SocksPort` → `127.0.0.1:9050`
- `arti proxy` SOCKS listen address (configurable; often `9150`)

See `umbra_tor::socks::{SocksProxy, TorMode}` and consider
[`libp2p-socks5`](https://crates.io/crates/libp2p-socks5) for the actual
transport. SOCKS5 CONNECT sends the **hostname** to the proxy, so there is no
local DNS leak and `.onion` works.

**Critical:** use SOCKS **instead of** clearnet TCP on that swarm, not beside
it. Mixing paths lets `identify` / advertised listen addrs correlate Tor and
clearnet identities.

## What works / what does not

| Capability | Status |
| --- | --- |
| Dial clearnet TCP over Tor exits | Yes (embedded arti or SOCKS) |
| Dial `/dns[4|6]/…/tcp/…` without local DNS | Yes |
| Dial `/onion3/…` (v3 onion services) | Yes if arti built with `onion-service-client` (enabled under `tor`) or SOCKS to a Tor that supports onions |
| Listen / host an onion service (inbound) | **Not in this spike.** arti can run onion services (`onion-service-service`), but long-lived stable `.onion` keys, mobile background limits, and libp2p `listen_on` integration are non-trivial. `TorTransport::listen_on` returns `MultiaddrNotSupported`. |
| UDP / QUIC transports over Tor | **No.** Tor is TCP-only. |
| WebRTC voice/video (`calls/`) over Tor | **No.** WebRTC needs UDP/ICE; keep calls on clearnet (still E2E via DTLS-SRTP) or accept “messaging over Tor, calls clearnet”. |
| Mobile battery / binary size | Embedded arti is large (many deps, SQLite dir cache, rustls). Prefer SOCKS + system Tor on desktop; treat embedded as desktop/advanced opt-in until measured on Android. |

## Privacy misuse warnings

Copied in spirit from `libp2p-community-tor` — read before shipping a “Tor mode” UI:

- A Tor transport does **not** make a normal libp2p node private by itself.
- `identify`, exchanged listen addresses, and any clearnet transport on the same
  swarm can link the Tor circuit to a real IP.
- Prefer: Tor-only dials, no clearnet listen addrs advertised, careful about
  what the app displays as “protected”.

## Wiring into Elite (`core/`)

Suggested once Elite’s transport builder exists (e.g. under `core/src/net/`):

1. Add to the workspace / `core` Cargo.toml:

   ```toml
   umbra-tor = { path = "tor", default-features = false }
   # optional:
   # umbra-tor = { path = "tor", features = ["tor"] }
   ```

2. Branch on a runtime setting mapped to `umbra_tor::TorMode`:

   ```rust
   match tor_mode {
       TorMode::Off => build_clearnet_tcp_quic(),
       TorMode::EmbeddedArti => {
           let tor = TorTransport::bootstrapped().await?;
           // upgrade with noise + yamux exactly like TCP; do NOT or() with clearnet
           upgrade(tor)
       }
       TorMode::Socks(proxy) => {
           // e.g. libp2p_socks5::SocksTransport::new(proxy.listen)
           upgrade(socks)
       }
   }
   ```

3. Keep `calls/` WebRTC on its own path; do not promise Tor for A/V in the UI
   when Tor mode is on.

4. Feature-gate Flutter/FFI: expose a `torAvailable` / `torMode` to Copilot’s
   `app/` so the setting can hide itself on builds without `tor`.

## How to build / test this crate

**Toolchain:** `rust-version = "1.88"` (libp2p's idna/icu transitive deps). On the Umbra
dev box use rustup's stable (`~/.cargo/bin` ahead of `/usr/bin`, which may still be 1.85.1):

```bash
export PATH="$HOME/.cargo/bin:$PATH"
cd core/tor
cargo test                  # stub + multiaddr + SOCKS tests (no arti)
cargo check --features tor  # pulls arti 0.35; needs network on first build
cargo run --example dial_info -- /dns4/example.org/tcp/443
```

No secrets belong here. arti’s state/cache dirs are local to the machine running
the client.

## Open questions for Elite / Copilot / Gem

- Workspace rustc/MSRV: stay on 1.85 (current pins) or jump to ≥1.88 and unlock
  libp2p 0.56+ / newer arti?
- Product decision: Tor for messaging only, or delay Tor UI until onion **listen**
  exists (true peer reachability without clearnet/relay)?
- Android: measure arti APK delta before enabling `tor` in release builds;
  otherwise ship SOCKS mode that expects Orbot / similar.
- Gem (`calls/`): document in UX that calls leave Tor.
