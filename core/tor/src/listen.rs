//! **EXPERIMENTAL** inbound onion-service listening (cargo feature `tor-listen`).
//!
//! arti's onion-service *hosting* support (`onion-service-service`,
//! `tor-hsservice`) is itself marked experimental upstream: expect API churn
//! between arti releases, reachability delays, and missing hardening (no
//! client authorization, limited DoS protection). Do not advertise this to
//! users as production-ready.
//!
//! # Listen address convention
//!
//! A peer usually does not know its own `.onion` address before the identity
//! key exists (it is generated on first launch and persisted in the keystore).
//! [`TorTransport`](crate::TorTransport)'s `listen_on` therefore accepts:
//!
//! - **Wildcard** `/onion3/<35 zero bytes>:<port>` built with
//!   [`onion_listen_addr`]: "host *my* onion service on virtual port `port`".
//! - **Exact** `/onion3/<own address>:<port>`: same, but the transport checks
//!   that the persisted identity really is that address (else it refuses).
//!
//! The transport then reports the real address via
//! `TransportEvent::NewAddress`.
//!
//! # Key storage
//!
//! Identity keys live **only** in arti's native keystore under
//! `<state_dir>/keystore/`, where `state_dir` comes from
//! [`TorConfig`](crate::TorConfig). Listening is refused when the transport was
//! built without custom dirs, so keys never land in an unexpected platform
//! default location. This crate never reads, copies, or logs key material, and
//! logs only the nickname and port — not even the (public) onion address.

use crate::TorError;
use arti_client::config::onion_service::{OnionServiceConfig, OnionServiceConfigBuilder};
use libp2p::multiaddr::{Multiaddr, Protocol};
use safelog::DisplayRedacted as _;
use tor_hsservice::{HsId, HsNickname};

/// The all-zero onion3 payload used as the "my own service" wildcard.
const WILDCARD_HASH: [u8; 35] = [0u8; 35];

/// Build the wildcard listen multiaddr for virtual port `port`.
///
/// ```
/// let addr = umbra_tor::onion_listen_addr(4001);
/// assert!(addr.to_string().starts_with("/onion3/"));
/// ```
pub fn onion_listen_addr(port: u16) -> Multiaddr {
    Multiaddr::empty().with(Protocol::Onion3((WILDCARD_HASH, port).into()))
}

/// What a `listen_on` multiaddr asks for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OnionListenRequest {
    /// Host our own service (whatever its address) on this virtual port.
    Wildcard { port: u16 },
    /// Host our own service; it must have exactly this address.
    Exact { addr: Multiaddr, port: u16 },
}

impl OnionListenRequest {
    pub fn port(&self) -> u16 {
        match self {
            Self::Wildcard { port } | Self::Exact { port, .. } => *port,
        }
    }
}

/// Parse a listen multiaddr. Only a bare `/onion3/…:port` is accepted (no
/// trailing protocols such as `/p2p/…`); everything else is `None`.
pub fn parse_onion_listen_addr(addr: &Multiaddr) -> Option<OnionListenRequest> {
    let mut it = addr.iter();
    let Protocol::Onion3(onion) = it.next()? else {
        return None;
    };
    if it.next().is_some() {
        return None;
    }
    let port = onion.port();
    if port == 0 {
        return None;
    }
    if *onion.hash() == WILDCARD_HASH {
        Some(OnionListenRequest::Wildcard { port })
    } else {
        Some(OnionListenRequest::Exact {
            addr: addr.clone(),
            port,
        })
    }
}

/// Validate an onion service nickname with arti's rules.
pub fn validate_nickname(nickname: &str) -> Result<HsNickname, TorError> {
    HsNickname::new(nickname.to_owned()).map_err(|_| TorError::InvalidNickname(nickname.to_owned()))
}

/// Build the arti onion-service config for `nickname` (defaults otherwise).
pub fn build_service_config(nickname: &str) -> Result<OnionServiceConfig, TorError> {
    let nickname = validate_nickname(nickname)?;
    Ok(OnionServiceConfigBuilder::default()
        .nickname(nickname)
        .build()?)
}

/// Convert our service's [`HsId`] plus virtual port into an `/onion3` multiaddr.
///
/// The address is public information, but it is still never logged here.
pub fn hsid_to_multiaddr(hsid: &HsId, port: u16) -> Option<Multiaddr> {
    let full = hsid.display_unredacted().to_string();
    let b32 = full.strip_suffix(".onion")?;
    format!("/onion3/{b32}:{port}").parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    const TORPROJECT: &str = "2gzyxa5ihm7nsggfxnu52rck2vv4rvmdlkiu3zzui5du4xyclen53wid";

    #[test]
    fn wildcard_roundtrip() {
        let a = onion_listen_addr(4001);
        assert_eq!(
            parse_onion_listen_addr(&a),
            Some(OnionListenRequest::Wildcard { port: 4001 })
        );
    }

    #[test]
    fn exact_addr_parsed() {
        let a: Multiaddr = format!("/onion3/{TORPROJECT}:80").parse().unwrap();
        let req = parse_onion_listen_addr(&a).unwrap();
        assert_eq!(req.port(), 80);
        assert!(matches!(req, OnionListenRequest::Exact { .. }));
    }

    #[test]
    fn rejects_non_onion_and_suffixes() {
        for s in [
            "/ip4/0.0.0.0/tcp/4001",
            "/dns4/example.org/tcp/443",
            "/ip4/1.2.3.4/udp/4001/quic-v1",
        ] {
            let a: Multiaddr = s.parse().unwrap();
            assert_eq!(parse_onion_listen_addr(&a), None, "{s}");
        }
        let with_p2p: Multiaddr = format!(
            "/onion3/{TORPROJECT}:80/p2p/12D3KooWD3eckifWpRn9wQpMG9R9hX3sD158z7EqHWmweQAJU5SA"
        )
        .parse()
        .unwrap();
        assert_eq!(parse_onion_listen_addr(&with_p2p), None);
        assert_eq!(parse_onion_listen_addr(&onion_listen_addr(0)), None);
    }

    #[test]
    fn nickname_validation() {
        assert!(validate_nickname("umbra").is_ok());
        assert!(validate_nickname("umbra-peer_2").is_ok());
        for bad in ["", "has space", "../escape", "slash/name", "Umbra", "-lead"] {
            assert!(
                matches!(validate_nickname(bad), Err(TorError::InvalidNickname(_))),
                "{bad:?} should be rejected"
            );
        }
    }

    #[test]
    fn service_config_builds() {
        build_service_config("umbra").expect("service config");
        assert!(build_service_config("bad name").is_err());
    }

    #[test]
    fn hsid_maps_to_onion3_multiaddr() {
        let hsid: HsId = format!("{TORPROJECT}.onion").parse().unwrap();
        let a = hsid_to_multiaddr(&hsid, 4001).unwrap();
        assert_eq!(a.to_string(), format!("/onion3/{TORPROJECT}:4001"));
        // And it round-trips into an exact listen request.
        assert_eq!(
            parse_onion_listen_addr(&a),
            Some(OnionListenRequest::Exact {
                addr: a.clone(),
                port: 4001
            })
        );
    }

    #[test]
    fn redacted_display_hides_prefix() {
        let hsid: HsId = format!("{TORPROJECT}.onion").parse().unwrap();
        let redacted = hsid.display_redacted().to_string();
        assert!(!redacted.contains(&TORPROJECT[..20]), "{redacted}");
    }
}
