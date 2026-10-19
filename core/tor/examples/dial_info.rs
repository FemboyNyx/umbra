//! Print how a multiaddr would be dialed over Tor / SOCKS (no network I/O).
//!
//! ```bash
//! cargo run --example dial_info -- /dns4/example.org/tcp/443
//! cargo run --example dial_info -- /onion3/2gzyxa5ihm7nsggfxnu52rck2vv4rvmdlkiu3zzui5du4xyclen53wid:80
//! ```

use std::env;
use umbra_tor::{
    multiaddr_to_socks_dest, multiaddr_to_target, SocksDest, SocksProxy, TorMode, TorTarget,
};

fn main() {
    let raw = env::args()
        .nth(1)
        .unwrap_or_else(|| "/dns4/example.org/tcp/443".into());
    let addr: libp2p::Multiaddr = raw.parse().expect("multiaddr");
    match multiaddr_to_target(&addr) {
        Some(TorTarget::Ip(sa)) => println!("exit to IP {sa} (exit relay sees the IP)"),
        Some(TorTarget::Host(h, p)) => {
            println!("exit to host {h}:{p} (resolved by exit, no local DNS)")
        }
        Some(TorTarget::Onion(h, p)) => println!("onion service {h}:{p}"),
        None => {
            eprintln!("not dialable over Tor (need TCP; no QUIC/WebRTC/ws): {addr}");
            std::process::exit(1);
        }
    }
    match multiaddr_to_socks_dest(&addr) {
        Some(SocksDest::Ip(sa)) => println!("SOCKS ATYP ip → {sa}"),
        Some(SocksDest::Domain { host, port }) => {
            println!("SOCKS ATYP domain → {host}:{port} (proxy resolves)")
        }
        None => unreachable!("target mapped but socks dest missing"),
    }
    println!(
        "suggested mode: {:?}",
        TorMode::Socks(SocksProxy::tor_daemon())
    );
}
