//! Optional Tor routing for Umbra.
//!
//! # Modes
//!
//! | Build | What you get |
//! | --- | --- |
//! | default | [`multiaddr_to_target`], [`socks`] (`TorMode`, [`SocksDest`]), [`TorConfig`] |
//! | `--features tor` | plus [`TorTransport`]: dial-only [`libp2p::core::Transport`] via embedded arti |
//! | `--features tor-listen` | **EXPERIMENTAL**: plus inbound v3 onion-service listening (`listen_on` with `/onion3`) |
//!
//! Inbound onion listening is gated behind the off-by-default `tor-listen`
//! feature because arti's onion-service hosting is experimental upstream; see
//! the `listen` module and `docs/tor.md`.
//!
//! `libp2p-community-tor` was evaluated and **not** used: it pins arti ^0.24 and
//! libp2p ^0.53 and has had no meaningful maintenance since late 2024. This crate
//! embeds arti directly (or points at an external SOCKS5 proxy) instead.

use libp2p::multiaddr::{Multiaddr, Protocol};
use std::net::SocketAddr;

pub mod config;
#[cfg(feature = "tor-listen")]
pub mod listen;
pub mod socks;
pub use config::TorConfig;
#[cfg(feature = "tor-listen")]
pub use config::DEFAULT_ONION_NICKNAME;
#[cfg(feature = "tor-listen")]
pub use listen::{onion_listen_addr, parse_onion_listen_addr, OnionListenRequest};
pub use socks::{multiaddr_to_socks_dest, target_to_socks_dest, SocksDest, SocksProxy, TorMode};

/// Where a multiaddr points, in a form Tor can dial.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TorTarget {
    /// Raw IP: Tor exits to it (the exit relay sees the IP).
    Ip(SocketAddr),
    /// Hostname resolved by the exit relay, never locally (no DNS leak).
    Host(String, u16),
    /// A v3 onion service: `<56 chars>.onion` and port.
    Onion(String, u16),
}

#[derive(Debug, thiserror::Error)]
pub enum TorError {
    #[error("multiaddr is not dialable over Tor: {0}")]
    Unsupported(Multiaddr),
    /// Custom state/cache dirs must both be set, or neither (arti defaults).
    #[error("TorConfig: set both state_dir and cache_dir, or neither")]
    IncompleteDirs,
    #[cfg(feature = "tor")]
    #[error("arti config: {0}")]
    Config(#[from] arti_client::config::ConfigBuildError),
    #[cfg(feature = "tor")]
    #[error("arti: {0}")]
    Arti(#[from] arti_client::Error),
    /// EXPERIMENTAL (`tor-listen`): nickname rejected by arti's `HsNickname` rules.
    #[cfg(feature = "tor-listen")]
    #[error("invalid onion service nickname: {0:?}")]
    InvalidNickname(String),
    /// EXPERIMENTAL (`tor-listen`): listening needs explicit state/cache dirs so
    /// onion keys are only stored in the app-controlled keystore.
    #[cfg(feature = "tor-listen")]
    #[error("onion listening requires TorConfig with custom state_dir and cache_dir")]
    ListenRequiresCustomDirs,
    /// EXPERIMENTAL (`tor-listen`): an exact `/onion3` listen address did not
    /// match this node's persisted onion identity.
    #[cfg(feature = "tor-listen")]
    #[error("listen address does not match this node's onion identity")]
    OnionAddressMismatch,
    /// EXPERIMENTAL (`tor-listen`): only one onion listener per transport (one
    /// service per nickname; arti locks its state while it runs).
    #[cfg(feature = "tor-listen")]
    #[error("an onion service listener is already active on this transport")]
    OnionListenerActive,
    /// EXPERIMENTAL (`tor-listen`): the service's identity key is not in the keystore.
    #[cfg(feature = "tor-listen")]
    #[error("onion service identity key not found in keystore")]
    OnionIdentityMissing,
    /// EXPERIMENTAL (`tor-listen`): accepting an incoming onion stream failed.
    /// Boxed: `ClientError` is large and would bloat every `Result<_, TorError>`.
    #[cfg(feature = "tor-listen")]
    #[error("onion service stream: {0}")]
    OnionStream(Box<tor_hsservice::ClientError>),
}

#[cfg(feature = "tor-listen")]
impl From<tor_hsservice::ClientError> for TorError {
    fn from(e: tor_hsservice::ClientError) -> Self {
        Self::OnionStream(Box::new(e))
    }
}

/// Map a libp2p multiaddr to a Tor target.
///
/// Returns `None` for anything that is not TCP-shaped (QUIC/UDP, WebRTC, `/ws`,
/// …). Tor only carries TCP streams.
pub fn multiaddr_to_target(addr: &Multiaddr) -> Option<TorTarget> {
    let mut it = addr.iter();
    let target = match it.next()? {
        Protocol::Ip4(ip) => match it.next()? {
            Protocol::Tcp(port) => TorTarget::Ip(SocketAddr::new(ip.into(), port)),
            _ => return None,
        },
        Protocol::Ip6(ip) => match it.next()? {
            Protocol::Tcp(port) => TorTarget::Ip(SocketAddr::new(ip.into(), port)),
            _ => return None,
        },
        Protocol::Dns(h) | Protocol::Dns4(h) | Protocol::Dns6(h) => match it.next()? {
            Protocol::Tcp(port) => TorTarget::Host(h.to_string(), port),
            _ => return None,
        },
        Protocol::Onion3(onion) => {
            let host = format!("{}.onion", data_encoding_base32_lower(onion.hash()));
            TorTarget::Onion(host, onion.port())
        }
        _ => return None,
    };
    // Allow a trailing /p2p/<peer id>; reject anything else (e.g. /ws, /quic).
    match it.next() {
        None => Some(target),
        Some(Protocol::P2p(_)) if it.next().is_none() => Some(target),
        _ => None,
    }
}

/// RFC 4648 base32, lowercase, no padding (onion v3 hostnames).
fn data_encoding_base32_lower(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 32] = b"abcdefghijklmnopqrstuvwxyz234567";
    let mut out = String::with_capacity((bytes.len() * 8).div_ceil(5));
    let (mut buf, mut bits) = (0u32, 0u32);
    for &b in bytes {
        buf = (buf << 8) | u32::from(b);
        bits += 8;
        while bits >= 5 {
            bits -= 5;
            out.push(ALPHABET[((buf >> bits) & 31) as usize] as char);
        }
    }
    if bits > 0 {
        out.push(ALPHABET[((buf << (5 - bits)) & 31) as usize] as char);
    }
    out
}

#[cfg(feature = "tor")]
mod transport;
#[cfg(feature = "tor")]
pub use transport::{TorStream, TorTransport};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_tcp_addrs() {
        let a: Multiaddr = "/ip4/1.2.3.4/tcp/4001".parse().unwrap();
        assert_eq!(
            multiaddr_to_target(&a),
            Some(TorTarget::Ip("1.2.3.4:4001".parse().unwrap()))
        );
        let a: Multiaddr = "/dns4/example.org/tcp/443".parse().unwrap();
        assert_eq!(
            multiaddr_to_target(&a),
            Some(TorTarget::Host("example.org".into(), 443))
        );
    }

    #[test]
    fn allows_trailing_peer_id() {
        let a: Multiaddr =
            "/dns/example.org/tcp/4001/p2p/12D3KooWD3eckifWpRn9wQpMG9R9hX3sD158z7EqHWmweQAJU5SA"
                .parse()
                .unwrap();
        assert!(multiaddr_to_target(&a).is_some());
    }

    #[test]
    fn rejects_non_tcp() {
        for s in [
            "/ip4/1.2.3.4/udp/4001/quic-v1",
            "/ip4/1.2.3.4/tcp/4001/ws",
            "/ip4/1.2.3.4",
        ] {
            let a: Multiaddr = s.parse().unwrap();
            assert_eq!(multiaddr_to_target(&a), None, "{s}");
        }
    }

    #[test]
    fn maps_onion3() {
        let host = "2gzyxa5ihm7nsggfxnu52rck2vv4rvmdlkiu3zzui5du4xyclen53wid";
        let a: Multiaddr = format!("/onion3/{host}:80").parse().unwrap();
        assert_eq!(
            multiaddr_to_target(&a),
            Some(TorTarget::Onion(format!("{host}.onion"), 80))
        );
    }
}
