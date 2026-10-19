//! SOCKS5 fallback design (no arti embed).
//!
//! When the `tor` feature is off — or when embedding arti is too heavy for a
//! target (Android APK size, battery) — Umbra can still route outbound dials
//! through an external Tor by speaking SOCKS5 to:
//!
//! - system `tor`'s `SocksPort` (typically `127.0.0.1:9050`)
//! - `arti proxy`'s SOCKS listen address (configurable)
//! - Orbot / similar on Android
//!
//! ## Why SOCKS5 and not a local DNS lookup
//!
//! SOCKS5 CONNECT carries the **hostname** in the request. The proxy (Tor)
//! resolves it. That means:
//!
//! - no local DNS query that would leak the destination
//! - `.onion` names work at all (they have no public DNS)
//!
//! ## `/onion3` → SOCKS domain
//!
//! Use [`multiaddr_to_socks_dest`] / [`target_to_socks_dest`]: an `/onion3/…`
//! multiaddr becomes a SOCKS5 **domain** target (`ATYP=0x03`) of
//! `<56chars>.onion:port`, never a local resolve.
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

use crate::{multiaddr_to_target, TorTarget};

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
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum TorMode {
    /// No Tor. Clearnet transports only.
    #[default]
    Off,
    /// Embed arti (`umbra-tor` built with `--features tor`) and use [`crate::TorTransport`].
    EmbeddedArti,
    /// Dial through an external SOCKS5 proxy (system tor or `arti proxy`).
    Socks(SocksProxy),
}

/// Destination to put in a SOCKS5 CONNECT request.
///
/// Domain variants **must** be sent as ATYP domain (0x03) so the proxy does DNS
/// / onion resolution — never resolve locally first.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SocksDest {
    /// Literal IP (ATYP IPv4/IPv6). Exit relay still sees the IP.
    Ip(SocketAddr),
    /// Hostname or `.onion` (ATYP domain) and port.
    Domain { host: String, port: u16 },
}

impl SocksDest {
    /// Host string for ATYP domain, if any.
    pub fn domain_host(&self) -> Option<&str> {
        match self {
            Self::Domain { host, .. } => Some(host.as_str()),
            Self::Ip(_) => None,
        }
    }

    pub fn port(&self) -> u16 {
        match self {
            Self::Ip(sa) => sa.port(),
            Self::Domain { port, .. } => *port,
        }
    }
}

/// Map a [`TorTarget`] to a SOCKS5 destination.
///
/// `/onion3` targets become `Domain { host: "<pubkey>.onion", port }` so the
/// Tor SOCKS proxy handles the onion circuit (remote “DNS”).
pub fn target_to_socks_dest(target: &TorTarget) -> SocksDest {
    match target {
        TorTarget::Ip(sa) => SocksDest::Ip(*sa),
        TorTarget::Host(h, p) | TorTarget::Onion(h, p) => SocksDest::Domain {
            host: h.clone(),
            port: *p,
        },
    }
}

/// Map a libp2p multiaddr to a SOCKS5 destination.
///
/// Returns `None` for non-TCP shapes (QUIC, `/ws`, …), same as
/// [`crate::multiaddr_to_target`].
pub fn multiaddr_to_socks_dest(addr: &Multiaddr) -> Option<SocksDest> {
    multiaddr_to_target(addr).map(|t| target_to_socks_dest(&t))
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

    #[test]
    fn tor_mode_default_is_off() {
        assert_eq!(TorMode::default(), TorMode::Off);
    }

    #[test]
    fn onion3_maps_to_socks_domain() {
        let host = "2gzyxa5ihm7nsggfxnu52rck2vv4rvmdlkiu3zzui5du4xyclen53wid";
        let a: Multiaddr = format!("/onion3/{host}:80").parse().unwrap();
        let dest = multiaddr_to_socks_dest(&a).expect("onion3 dialable");
        let expected_host = format!("{host}.onion");
        assert_eq!(
            dest,
            SocksDest::Domain {
                host: expected_host.clone(),
                port: 80,
            }
        );
        assert_eq!(dest.domain_host(), Some(expected_host.as_str()));
        assert!(dest.domain_host().unwrap().ends_with(".onion"));
    }

    #[test]
    fn dns_maps_to_socks_domain() {
        let a: Multiaddr = "/dns4/example.org/tcp/443".parse().unwrap();
        assert_eq!(
            multiaddr_to_socks_dest(&a),
            Some(SocksDest::Domain {
                host: "example.org".into(),
                port: 443,
            })
        );
    }

    #[test]
    fn ip_maps_to_socks_ip() {
        let a: Multiaddr = "/ip4/1.2.3.4/tcp/4001".parse().unwrap();
        assert_eq!(
            multiaddr_to_socks_dest(&a),
            Some(SocksDest::Ip("1.2.3.4:4001".parse().unwrap()))
        );
    }

    #[test]
    fn quic_not_socks() {
        let a: Multiaddr = "/ip4/1.2.3.4/udp/4001/quic-v1".parse().unwrap();
        assert_eq!(multiaddr_to_socks_dest(&a), None);
    }
}
