//! Print how a multiaddr would be dialed over Tor (no network I/O).
//!
//! ```bash
//! cargo run --example dial_info -- /dns4/example.org/tcp/443
//! ```

use std::env;
use umbra_tor::{multiaddr_to_target, SocksProxy, TorMode, TorTarget};

fn main() {
    let raw = env::args()
        .nth(1)
        .unwrap_or_else(|| "/dns4/example.org/tcp/443".into());
    let addr: libp2p::Multiaddr = raw.parse().expect("multiaddr");
    match multiaddr_to_target(&addr) {
        Some(TorTarget::Ip(sa)) => println!("exit to IP {sa} (exit relay sees the IP)"),
        Some(TorTarget::Host(h, p)) => println!("exit to host {h}:{p} (resolved by exit, no local DNS)"),
        Some(TorTarget::Onion(h, p)) => println!("onion service {h}:{p}"),
        None => {
            eprintln!("not dialable over Tor (need TCP; no QUIC/WebRTC/ws): {addr}");
            std::process::exit(1);
        }
    }
    println!("suggested mode: {:?}", TorMode::Socks(SocksProxy::tor_daemon()));
}
