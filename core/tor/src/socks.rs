//! SOCKS5 fallback design (no arti embed).
//!
//! When the `tor` feature is off — or when embedding arti is too heavy for a
//! target (Android APK size, battery) — Umbra can still route outbound dials
//! through an external Tor by speaking SOCKS5 to:
//!
//! - system `tor`'s `SocksPort` (typically `127.0.0.1:9050`)
//! - `arti proxy`'s SOCKS listen address (configurable)
//!
//! ## Why SOCKS5 and not a local DNS lookup
//!
//! SOCKS5 CONNECT carries the **hostname** in the request. The proxy (Tor)
//! resolves it. That means:
//!
//! - no local DNS query that would leak the destination
//! - `.onion` names work at all (they have no public DNS)
//!
//! ## Recommended wiring for Elite
//!
//! Prefer an existing outbound-only crate such as [`libp2p-socks5`]
//! (https://crates.io/crates/libp2p-socks5) pointed at the proxy address,
//! composed **instead of** clearnet TCP — never as a silent fallback beside it.
//! Mixing Tor and clearnet transports on one swarm lets `identify` (and any
//! `add_external_address`) correlate the two paths.
//!
//! This module only carries the config + multiaddr helpers so `core/` can grow
//! a Tor mode without pulling arti into every build.

use std::net::SocketAddr;

use libp2p::Multiaddr;

/// Where an external Tor SOCKS5 proxy is listening.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SocksProxy {
    /// Usually `127.0.0.1:9050` (tor) or whatever `arti proxy` was configured with.
    pub listen: SocketAddr,
}

impl SocksProxy {
    pub const DEFAULT_TOR_DAEMON: &'static str = "127.0.0.1:9050";
    pub const DEFAULT_ARTI_PROXY: &'static str = "127.0.0.1:9150";

    pub fn tor_daemon() -> Self {
        Self {
            listen: Self::DEFAULT_TOR_DAEMON.parse().expect("static addr"),
        }
    }

    pub fn arti_proxy() -> Self {
        Self {
            listen: Self::DEFAULT_ARTI_PROXY.parse().expect("static addr"),
        }
    }

    /// Multiaddr form of the proxy itself (for logging / UI), not a dial target.
    pub fn proxy_multiaddr(&self) -> Multiaddr {
        let mut addr = Multiaddr::empty();
        match self.listen {
            SocketAddr::V4(v4) => {
                addr.push(libp2p::multiaddr::Protocol::Ip4(*v4.ip()));
                addr.push(libp2p::multiaddr::Protocol::Tcp(v4.port()));
            }
            SocketAddr::V6(v6) => {
                addr.push(libp2p::multiaddr::Protocol::Ip6(*v6.ip()));
                addr.push(libp2p::multiaddr::Protocol::Tcp(v6.port()));
            }
        }
        addr
    }
}

/// How Umbra should obtain Tor connectivity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TorMode {
    /// No Tor. Clearnet transports only.
    Off,
    /// Embed arti (`umbra-tor` built with `--features tor`) and use [`crate::TorTransport`].
    EmbeddedArti,
    /// Dial through an external SOCKS5 proxy (system tor or `arti proxy`).
    Socks(SocksProxy),
}

impl Default for TorMode {
    fn default() -> Self {
        Self::Off
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_proxy_addrs_parse() {
        let p = SocksProxy::tor_daemon();
        assert_eq!(p.listen.port(), 9050);
        let a = p.proxy_multiaddr();
        assert!(a.to_string().contains("127.0.0.1"));
        assert!(a.to_string().contains("9050"));
    }
}
