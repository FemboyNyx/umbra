# umbra-tor

Optional Tor routing spike for Umbra. Isolated under `core/tor/` so Elite's
`core/` crate can take a soft dependency later without owning this code yet.

- **Default build**: multiaddr → Tor-target helpers + SOCKS5 fallback design.
- **`--features tor`**: embeds `arti-client` 0.35 and exposes `TorTransport`,
  a dial-only `libp2p::core::Transport`.

See [`docs/tor.md`](../../docs/tor.md) for feasibility, limits, and wiring notes.

```bash
cargo test
cargo check --features tor   # downloads arti; needs network + a few minutes
```
