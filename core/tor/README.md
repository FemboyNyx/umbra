# umbra-tor

Optional Tor routing spike for Umbra. Isolated under `core/tor/` so Elite's
`core/` workspace can depend on it without owning this code.

- **Default build**: multiaddr → [`TorTarget`] / [`SocksDest`], [`TorMode`], [`TorConfig`].
- **`--features tor`**: embeds `arti-client` 0.35 and exposes [`TorTransport`]
  (dial-only `libp2p::core::Transport`), with configurable state/cache dirs for Android.

See [`docs/tor.md`](../../docs/tor.md).

## Enable in Elite's `core/` workspace

In `core/Cargo.toml` (already stubbed by Elite):

1. Uncomment the workspace member:

   ```toml
   [workspace]
   members = [".", "tor"]
   ```

2. Uncomment the dependency and feature:

   ```toml
   umbra-tor = { path = "tor", optional = true, features = ["tor"] }

   [features]
   tor = ["dep:umbra-tor"]
   ```

3. On Android, pass app dirs before bootstrap:

   ```rust
   let cfg = umbra_tor::TorConfig::new()
       .state_dir(app_files_dir.join("arti-state"))
       .cache_dir(app_cache_dir.join("arti-cache"));
   let tor = umbra_tor::TorTransport::bootstrapped_with(cfg).await?;
   ```

## Develop

```bash
export PATH="$HOME/.cargo/bin:$PATH"   # rustup stable (>= 1.89)
cd core/tor
cargo fmt
cargo test
cargo test --features tor
cargo clippy --all-targets -- -D warnings
cargo clippy --all-targets --features tor -- -D warnings
```

MSRV: **1.89** (matches Elite / vodozemac 0.11). arti 0.35 shares rusqlite **0.37**;
`tor` feature enables arti `static-sqlite` (no system libsqlite3)
with `umbra_core` — bump them together.
