//! Minimal dial-only SOCKS5 (RFC 1928, no auth) transport for Tor mode.
//!
//! Used when `CoreConfig.tor_socks` is set (system `tor`, Orbot, `arti proxy`).
//! Hostnames are sent to the proxy unresolved (no local DNS leak). Listening
//! is not supported. This is the ONLY base transport in Tor mode — it is never
//! combined with clearnet TCP/QUIC.

use std::net::{IpAddr, SocketAddr};
use std::pin::Pin;
use std::task::{Context, Poll};

use futures::future::BoxFuture;
use libp2p::core::transport::{DialOpts, ListenerId, Transport, TransportError, TransportEvent};
use libp2p::core::Endpoint;
use libp2p::multiaddr::Protocol;
use libp2p::Multiaddr;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio_util::compat::{Compat, TokioAsyncReadCompatExt};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SocksTarget {
    Ip(SocketAddr),
    Host(String, u16),
}

/// Map a TCP-shaped multiaddr (optionally ending in `/p2p/..`) to a target.
pub fn target_of(addr: &Multiaddr) -> Option<SocksTarget> {
    let mut it = addr.iter();
    let t = match it.next()? {
        Protocol::Ip4(ip) => match it.next()? {
            Protocol::Tcp(p) => SocksTarget::Ip(SocketAddr::new(IpAddr::V4(ip), p)),
            _ => return None,
        },
        Protocol::Ip6(ip) => match it.next()? {
            Protocol::Tcp(p) => SocksTarget::Ip(SocketAddr::new(IpAddr::V6(ip), p)),
            _ => return None,
        },
        Protocol::Dns(h) | Protocol::Dns4(h) | Protocol::Dns6(h) => match it.next()? {
            Protocol::Tcp(p) => SocksTarget::Host(h.to_string(), p),
            _ => return None,
        },
        _ => return None,
    };
    match it.next() {
        None => Some(t),
        Some(Protocol::P2p(_)) if it.next().is_none() => Some(t),
        _ => None,
    }
}

#[derive(Clone)]
pub struct SocksTransport {
    proxy: SocketAddr,
}

impl SocksTransport {
    pub fn new(proxy: SocketAddr) -> Self {
        Self { proxy }
    }
}

pub async fn socks5_connect(proxy: SocketAddr, target: &SocksTarget) -> std::io::Result<TcpStream> {
    use std::io::{Error, ErrorKind};
    let mut s = TcpStream::connect(proxy).await?;
    s.write_all(&[5, 1, 0]).await?;
    let mut buf = [0u8; 2];
    s.read_exact(&mut buf).await?;
    if buf != [5, 0] {
        return Err(Error::new(
            ErrorKind::PermissionDenied,
            "SOCKS5 proxy requires unsupported auth",
        ));
    }
    let mut req = vec![5, 1, 0];
    let port = match target {
        SocksTarget::Ip(sa) => {
            match sa.ip() {
                IpAddr::V4(ip) => {
                    req.push(1);
                    req.extend_from_slice(&ip.octets());
                }
                IpAddr::V6(ip) => {
                    req.push(4);
                    req.extend_from_slice(&ip.octets());
                }
            }
            sa.port()
        }
        SocksTarget::Host(h, p) => {
            let hb = h.as_bytes();
            if hb.len() > 255 {
                return Err(Error::new(ErrorKind::InvalidInput, "hostname too long"));
            }
            req.push(3);
            req.push(hb.len() as u8);
            req.extend_from_slice(hb);
            *p
        }
    };
    req.extend_from_slice(&port.to_be_bytes());
    s.write_all(&req).await?;
    let mut head = [0u8; 4];
    s.read_exact(&mut head).await?;
    if head[0] != 5 || head[1] != 0 {
        return Err(Error::new(
            ErrorKind::ConnectionRefused,
            format!("SOCKS5 connect failed (code {})", head[1]),
        ));
    }
    let skip = match head[3] {
        1 => 4,
        4 => 16,
        3 => {
            let mut l = [0u8; 1];
            s.read_exact(&mut l).await?;
            l[0] as usize
        }
        _ => return Err(Error::new(ErrorKind::InvalidData, "bad SOCKS5 reply")),
    };
    let mut rest = vec![0u8; skip + 2];
    s.read_exact(&mut rest).await?;
    Ok(s)
}

impl Transport for SocksTransport {
    type Output = Compat<TcpStream>;
    type Error = std::io::Error;
    type ListenerUpgrade = futures::future::Pending<Result<Self::Output, Self::Error>>;
    type Dial = BoxFuture<'static, Result<Self::Output, Self::Error>>;

    fn listen_on(
        &mut self,
        _id: ListenerId,
        addr: Multiaddr,
    ) -> Result<(), TransportError<Self::Error>> {
        Err(TransportError::MultiaddrNotSupported(addr))
    }

    fn remove_listener(&mut self, _id: ListenerId) -> bool {
        false
    }

    fn dial(
        &mut self,
        addr: Multiaddr,
        opts: DialOpts,
    ) -> Result<Self::Dial, TransportError<Self::Error>> {
        // No hole punching (it would need a direct path).
        if opts.role == Endpoint::Listener {
            return Err(TransportError::MultiaddrNotSupported(addr));
        }
        let Some(target) = target_of(&addr) else {
            return Err(TransportError::MultiaddrNotSupported(addr));
        };
        let proxy = self.proxy;
        Ok(Box::pin(async move {
            Ok(socks5_connect(proxy, &target).await?.compat())
        }))
    }

    fn poll(
        self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
    ) -> Poll<TransportEvent<Self::ListenerUpgrade, Self::Error>> {
        Poll::Pending
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::net::TcpListener;

    #[test]
    fn maps_targets() {
        let a: Multiaddr = "/dns4/example.org/tcp/443".parse().unwrap();
        assert_eq!(
            target_of(&a),
            Some(SocksTarget::Host("example.org".into(), 443))
        );
        let a: Multiaddr = "/ip4/1.2.3.4/udp/1/quic-v1".parse().unwrap();
        assert_eq!(target_of(&a), None);
    }

    /// Fake SOCKS5 proxy that checks the CONNECT request carries the hostname.
    #[tokio::test]
    async fn speaks_socks5() {
        let l = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let proxy = l.local_addr().unwrap();
        let srv = tokio::spawn(async move {
            let (mut s, _) = l.accept().await.unwrap();
            let mut b = [0u8; 3];
            s.read_exact(&mut b).await.unwrap();
            assert_eq!(b, [5, 1, 0]);
            s.write_all(&[5, 0]).await.unwrap();
            let mut h = [0u8; 5];
            s.read_exact(&mut h).await.unwrap();
            assert_eq!(&h[..4], &[5, 1, 0, 3]);
            let mut host = vec![0u8; h[4] as usize + 2];
            s.read_exact(&mut host).await.unwrap();
            assert_eq!(&host[..h[4] as usize], b"peer.onion");
            s.write_all(&[5, 0, 0, 1, 0, 0, 0, 0, 0, 0]).await.unwrap();
            s.write_all(b"ok").await.unwrap();
        });
        let mut s = socks5_connect(proxy, &SocksTarget::Host("peer.onion".into(), 80))
            .await
            .unwrap();
        let mut ok = [0u8; 2];
        s.read_exact(&mut ok).await.unwrap();
        assert_eq!(&ok, b"ok");
        srv.await.unwrap();
    }
}
