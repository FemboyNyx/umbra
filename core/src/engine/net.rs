//! libp2p swarm: transport selection, behaviours and the event loop task.
//!
//! Clearnet mode: TCP + QUIC + DNS, Noise + Yamux, relay v2 client, DCUtR,
//! optional mDNS.
//! Tor mode (`tor_socks` set, or cargo feature `tor` = embedded arti):
//! Tor dial-only transport + relay client
//! ONLY (never `or()`-ed with clearnet), no DNS resolver, no mDNS, no DCUtR,
//! identify hides listen addrs. Inbound reachability only via relay circuits.

use std::collections::{HashMap, HashSet};
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{anyhow, Result};
use futures::StreamExt;
use libp2p::core::upgrade::Version;
use libp2p::multiaddr::Protocol;
use libp2p::request_response::{self, OutboundRequestId, ProtocolSupport};
use libp2p::swarm::behaviour::toggle::Toggle;
use libp2p::swarm::dial_opts::{DialOpts, PeerCondition};
use libp2p::swarm::{NetworkBehaviour, SwarmEvent};
use libp2p::{
    dcutr, identify, identity, kad, mdns, noise, ping, relay, tcp, yamux, Multiaddr, PeerId,
    StreamProtocol, Swarm, Transport,
};
use tokio::sync::{mpsc, oneshot};

use crate::engine::core::Core;
use crate::engine::proto::*;
use crate::engine::socks::SocksTransport;

const REQUEST_TIMEOUT: Duration = Duration::from_secs(20);
const IDLE_TIMEOUT: Duration = Duration::from_secs(300);
pub const RETRY_INTERVAL: Duration = Duration::from_secs(15);

#[derive(NetworkBehaviour)]
pub struct Behaviour {
    relay: relay::client::Behaviour,
    ping: ping::Behaviour,
    identify: identify::Behaviour,
    kad: kad::Behaviour<kad::store::MemoryStore>,
    mdns: Toggle<mdns::tokio::Behaviour>,
    dcutr: Toggle<dcutr::Behaviour>,
    msg: request_response::cbor::Behaviour<MsgRequest, MsgResponse>,
    keys: request_response::cbor::Behaviour<KeysRequest, KeysResponse>,
}

type BoxErr = Box<dyn std::error::Error + Send + Sync>;

fn make_behaviour(
    key: &identity::Keypair,
    relay: relay::client::Behaviour,
    tor: bool,
    mdns_on: bool,
) -> Result<Behaviour, BoxErr> {
    let peer = key.public().to_peer_id();
    let rr_cfg = request_response::Config::default().with_request_timeout(REQUEST_TIMEOUT);
    let mut kad_cfg = kad::Config::new(StreamProtocol::new(KAD_PROTOCOL));
    kad_cfg.set_query_timeout(Duration::from_secs(30));
    let mut kad = kad::Behaviour::with_config(peer, kad::store::MemoryStore::new(peer), kad_cfg);
    if tor {
        kad.set_mode(Some(kad::Mode::Client));
    }
    let mdns = if mdns_on && !tor {
        Some(mdns::tokio::Behaviour::new(mdns::Config::default(), peer)?)
    } else {
        None
    };
    Ok(Behaviour {
        relay,
        ping: ping::Behaviour::default(),
        identify: identify::Behaviour::new(
            identify::Config::new(IDENTIFY_PROTOCOL.into(), key.public())
                .with_agent_version(format!("umbra-core/{}", env!("CARGO_PKG_VERSION")))
                .with_hide_listen_addrs(tor),
        ),
        kad,
        mdns: mdns.into(),
        dcutr: (!tor).then(|| dcutr::Behaviour::new(peer)).into(),
        msg: request_response::cbor::Behaviour::new(
            [(StreamProtocol::new(MSG_PROTOCOL), ProtocolSupport::Full)],
            rr_cfg.clone(),
        ),
        keys: request_response::cbor::Behaviour::new(
            [(StreamProtocol::new(KEYS_PROTOCOL), ProtocolSupport::Full)],
            rr_cfg,
        ),
    })
}

/// Onion service nickname (arti keystore namespace) for `tor-listen`.
/// Changing it changes the `.onion` address.
#[cfg(feature = "tor-listen")]
pub const TOR_ONION_NICKNAME: &str = "umbra";

/// Create `dir` (and parents) and restrict it to the owner (0700 on Unix);
/// arti's fs-mistrust refuses group/world-accessible state dirs.
#[cfg(feature = "tor")]
pub fn private_dir(dir: &std::path::Path) -> Result<()> {
    std::fs::create_dir_all(dir).map_err(|e| anyhow!("create {}: {e}", dir.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700))
            .map_err(|e| anyhow!("chmod 0700 {}: {e}", dir.display()))?;
    }
    Ok(())
}

pub enum TransportMode {
    Clearnet {
        mdns: bool,
    },
    /// Tor via an external SOCKS5 proxy.
    TorSocks(SocketAddr),
    /// Tor via embedded arti (`umbra-tor`, cargo feature `tor`), with
    /// app-private arti state / cache dirs (created 0700 by the caller).
    #[cfg(feature = "tor")]
    TorEmbedded {
        state_dir: std::path::PathBuf,
        cache_dir: std::path::PathBuf,
    },
}

impl TransportMode {
    pub fn is_tor(&self) -> bool {
        !matches!(self, TransportMode::Clearnet { .. })
    }
}

pub async fn build_swarm(
    keypair: identity::Keypair,
    mode: &TransportMode,
) -> Result<Swarm<Behaviour>> {
    let swarm_cfg = |c: libp2p::swarm::Config| c.with_idle_connection_timeout(IDLE_TIMEOUT);
    let swarm = match mode {
        TransportMode::Clearnet { mdns } => {
            let mdns = *mdns;
            libp2p::SwarmBuilder::with_existing_identity(keypair)
                .with_tokio()
                .with_tcp(
                    tcp::Config::default().nodelay(true),
                    noise::Config::new,
                    yamux::Config::default,
                )?
                .with_quic()
                .with_dns()?
                .with_relay_client(noise::Config::new, yamux::Config::default)?
                .with_behaviour(|key, relay| make_behaviour(key, relay, false, mdns))
                .map_err(|e| anyhow!("behaviour: {e}"))?
                .with_swarm_config(swarm_cfg)
                .build()
        }
        TransportMode::TorSocks(proxy) => {
            let proxy = *proxy;
            libp2p::SwarmBuilder::with_existing_identity(keypair)
                .with_tokio()
                .with_other_transport(|key| -> Result<_, BoxErr> {
                    Ok(SocksTransport::new(proxy)
                        .upgrade(Version::V1)
                        .authenticate(noise::Config::new(key)?)
                        .multiplex(yamux::Config::default()))
                })
                .map_err(|e| anyhow!("tor transport: {e}"))?
                .with_relay_client(noise::Config::new, yamux::Config::default)?
                .with_behaviour(|key, relay| make_behaviour(key, relay, true, false))
                .map_err(|e| anyhow!("behaviour: {e}"))?
                .with_swarm_config(swarm_cfg)
                .build()
        }
        #[cfg(feature = "tor")]
        TransportMode::TorEmbedded {
            state_dir,
            cache_dir,
        } => {
            // Explicit dirs: arti's XDG defaults don't exist on Android, and
            // the onion keystore (tor-listen) must never land in a default
            // location. The nickname is the keystore namespace; keep it stable.
            let tor_cfg = umbra_tor::TorConfig::new()
                .state_dir(state_dir)
                .cache_dir(cache_dir);
            #[cfg(feature = "tor-listen")]
            let tor_cfg = tor_cfg.onion_nickname(TOR_ONION_NICKNAME);
            let tor = umbra_tor::TorTransport::bootstrapped_with(tor_cfg)
                .await
                .map_err(|e| anyhow!("tor bootstrap: {e}"))?;
            libp2p::SwarmBuilder::with_existing_identity(keypair)
                .with_tokio()
                .with_other_transport(|key| -> Result<_, BoxErr> {
                    Ok(tor
                        .upgrade(Version::V1)
                        .authenticate(noise::Config::new(key)?)
                        .multiplex(yamux::Config::default()))
                })
                .map_err(|e| anyhow!("tor transport: {e}"))?
                .with_relay_client(noise::Config::new, yamux::Config::default)?
                .with_behaviour(|key, relay| make_behaviour(key, relay, true, false))
                .map_err(|e| anyhow!("behaviour: {e}"))?
                .with_swarm_config(swarm_cfg)
                .build()
        }
    };
    Ok(swarm)
}

pub enum Cmd {
    Msg {
        peer: PeerId,
        req: MsgRequest,
        reply: oneshot::Sender<Result<MsgResponse>>,
    },
    Keys {
        peer: PeerId,
        req: KeysRequest,
        reply: oneshot::Sender<Result<KeysResponse>>,
    },
    AddAddrs {
        peer: PeerId,
        addrs: Vec<Multiaddr>,
    },
    Stop,
}

/// Cloneable handle to the running swarm task.
#[derive(Clone)]
pub struct NetHandle {
    tx: mpsc::UnboundedSender<Cmd>,
}

impl NetHandle {
    pub async fn msg(&self, peer: PeerId, req: MsgRequest) -> Result<MsgResponse> {
        let (reply, rx) = oneshot::channel();
        self.tx
            .send(Cmd::Msg { peer, req, reply })
            .map_err(|_| anyhow!("network stopped"))?;
        rx.await.map_err(|_| anyhow!("network stopped"))?
    }

    pub async fn keys(&self, peer: PeerId, req: KeysRequest) -> Result<KeysResponse> {
        let (reply, rx) = oneshot::channel();
        self.tx
            .send(Cmd::Keys { peer, req, reply })
            .map_err(|_| anyhow!("network stopped"))?;
        rx.await.map_err(|_| anyhow!("network stopped"))?
    }

    pub fn add_addrs(&self, peer: PeerId, addrs: Vec<Multiaddr>) {
        let _ = self.tx.send(Cmd::AddAddrs { peer, addrs });
    }

    pub fn stop(&self) {
        let _ = self.tx.send(Cmd::Stop);
    }
}

pub struct NetConfig {
    pub mode: TransportMode,
    pub listen: Vec<Multiaddr>,
    pub bootstrap: Vec<Multiaddr>,
    pub relays: Vec<Multiaddr>,
    /// Known contact addresses at startup.
    pub known: Vec<(PeerId, Vec<Multiaddr>)>,
}

fn split_p2p(addr: &Multiaddr) -> Option<PeerId> {
    match addr.iter().last() {
        Some(Protocol::P2p(p)) => Some(p),
        _ => None,
    }
}

/// Build the swarm, start listening and spawn the event loop. Returns once
/// listeners are registered (addresses arrive asynchronously).
pub async fn spawn(
    core: Arc<Core>,
    keypair: identity::Keypair,
    cfg: NetConfig,
) -> Result<(NetHandle, tokio::task::JoinHandle<()>)> {
    let tor = cfg.mode.is_tor();
    let mut swarm = build_swarm(keypair, &cfg.mode).await?;

    if !tor {
        // Best effort per address (e.g. no IPv6 on this network); fail only
        // if nothing could be bound.
        let mut errors = Vec::new();
        for a in &cfg.listen {
            if let Err(e) = swarm.listen_on(a.clone()) {
                tracing::warn!(%a, "listen failed: {e}");
                errors.push(format!("{a}: {e}"));
            }
        }
        if !cfg.listen.is_empty() && errors.len() == cfg.listen.len() {
            anyhow::bail!("could not listen on any address: {}", errors.join("; "));
        }
    }
    let mut book: HashMap<PeerId, HashSet<Multiaddr>> = HashMap::new();
    for (p, addrs) in cfg.known {
        book.entry(p).or_default().extend(addrs);
    }
    for b in &cfg.bootstrap {
        if let Some(p) = split_p2p(b) {
            swarm.behaviour_mut().kad.add_address(&p, b.clone());
        } else {
            tracing::warn!(%b, "bootstrap addr without /p2p/<id> ignored");
        }
    }
    if !cfg.bootstrap.is_empty() {
        let _ = swarm.behaviour_mut().kad.bootstrap();
    }
    for r in &cfg.relays {
        if split_p2p(r).is_none() {
            tracing::warn!(%r, "relay addr without /p2p/<id> ignored");
            continue;
        }
        if let Err(e) = swarm.listen_on(r.clone().with(Protocol::P2pCircuit)) {
            tracing::warn!(%r, "relay listen failed: {e}");
        }
    }

    let (tx, rx) = mpsc::unbounded_channel();
    let task = NetTask {
        swarm,
        queued: HashMap::new(),
        core,
        rx,
        book,
        pending_msg: HashMap::new(),
        pending_keys: HashMap::new(),
    };
    let jh = tokio::spawn(task.run());
    Ok((NetHandle { tx }, jh))
}

enum Queued {
    Msg(MsgRequest, oneshot::Sender<Result<MsgResponse>>),
    Keys(KeysRequest, oneshot::Sender<Result<KeysResponse>>),
}

struct NetTask {
    swarm: Swarm<Behaviour>,
    /// Requests waiting for a dial we started (sent on ConnectionEstablished).
    queued: HashMap<PeerId, Vec<Queued>>,
    core: Arc<Core>,
    rx: mpsc::UnboundedReceiver<Cmd>,
    book: HashMap<PeerId, HashSet<Multiaddr>>,
    pending_msg: HashMap<OutboundRequestId, oneshot::Sender<Result<MsgResponse>>>,
    pending_keys: HashMap<OutboundRequestId, oneshot::Sender<Result<KeysResponse>>>,
}

impl NetTask {
    async fn run(mut self) {
        let mut tick = tokio::time::interval(RETRY_INTERVAL);
        tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        loop {
            tokio::select! {
                cmd = self.rx.recv() => match cmd {
                    None | Some(Cmd::Stop) => break,
                    Some(cmd) => self.on_cmd(cmd),
                },
                ev = self.swarm.select_next_some() => self.on_event(ev),
                _ = tick.tick() => self.core.on_tick(),
            }
        }
        for (_, q) in self.queued.drain() {
            for item in q {
                match item {
                    Queued::Msg(_, r) => drop(r.send(Err(anyhow!("network stopped")))),
                    Queued::Keys(_, r) => drop(r.send(Err(anyhow!("network stopped")))),
                }
            }
        }
        for (_, r) in self.pending_msg.drain() {
            let _ = r.send(Err(anyhow!("network stopped")));
        }
        for (_, r) in self.pending_keys.drain() {
            let _ = r.send(Err(anyhow!("network stopped")));
        }
        self.core.on_network_stopped();
    }

    /// Returns true if the request must wait for a dial we manage.
    fn ensure_dial(&mut self, peer: PeerId) -> bool {
        if self.swarm.is_connected(&peer) {
            return false;
        }
        if self.queued.contains_key(&peer) {
            return true; // dial already in flight
        }
        let addrs: Vec<Multiaddr> = self
            .book
            .get(&peer)
            .map(|s| s.iter().cloned().collect())
            .unwrap_or_default();
        if addrs.is_empty() {
            // Ask the DHT; request_response will still try behaviour-provided addresses.
            self.swarm.behaviour_mut().kad.get_closest_peers(peer);
            return false;
        }
        let opts = DialOpts::peer_id(peer)
            .addresses(addrs)
            .extend_addresses_through_behaviour()
            .condition(PeerCondition::DisconnectedAndNotDialing)
            .build();
        match self.swarm.dial(opts) {
            Ok(()) => {
                self.queued.entry(peer).or_default();
                true
            }
            Err(e) => {
                tracing::debug!(%peer, "dial: {e}");
                false
            }
        }
    }

    fn send_now(&mut self, peer: PeerId, q: Queued) {
        match q {
            Queued::Msg(req, reply) => {
                let id = self.swarm.behaviour_mut().msg.send_request(&peer, req);
                self.pending_msg.insert(id, reply);
            }
            Queued::Keys(req, reply) => {
                let id = self.swarm.behaviour_mut().keys.send_request(&peer, req);
                self.pending_keys.insert(id, reply);
            }
        }
    }

    fn submit(&mut self, peer: PeerId, q: Queued) {
        if self.ensure_dial(peer) {
            self.queued.entry(peer).or_default().push(q);
        } else {
            self.send_now(peer, q);
        }
    }

    fn release_queued(&mut self, peer: PeerId, connected: bool) {
        for q in self.queued.remove(&peer).unwrap_or_default() {
            if connected {
                self.send_now(peer, q);
            } else {
                let err = || anyhow!("could not connect to {peer}");
                match q {
                    Queued::Msg(_, r) => drop(r.send(Err(err()))),
                    Queued::Keys(_, r) => drop(r.send(Err(err()))),
                }
            }
        }
    }

    fn on_cmd(&mut self, cmd: Cmd) {
        match cmd {
            Cmd::Msg { peer, req, reply } => self.submit(peer, Queued::Msg(req, reply)),
            Cmd::Keys { peer, req, reply } => self.submit(peer, Queued::Keys(req, reply)),
            Cmd::AddAddrs { peer, addrs } => {
                self.book.entry(peer).or_default().extend(addrs);
            }
            Cmd::Stop => {}
        }
    }

    fn on_event(&mut self, ev: SwarmEvent<BehaviourEvent>) {
        match ev {
            SwarmEvent::NewListenAddr { address, .. } => {
                tracing::info!(%address, "listening");
                self.core
                    .on_listen_addr(address, true, self.swarm.connected_peers().count());
            }
            SwarmEvent::ExpiredListenAddr { address, .. } => {
                self.core
                    .on_listen_addr(address, false, self.swarm.connected_peers().count());
            }
            SwarmEvent::ConnectionEstablished {
                peer_id,
                num_established,
                ..
            } => {
                self.release_queued(peer_id, true);
                if num_established.get() == 1 {
                    self.core
                        .on_connected(peer_id, self.swarm.connected_peers().count());
                }
            }
            SwarmEvent::ConnectionClosed {
                peer_id,
                num_established,
                ..
            } => {
                if num_established == 0 {
                    self.core
                        .on_disconnected(peer_id, self.swarm.connected_peers().count());
                }
            }
            SwarmEvent::OutgoingConnectionError {
                peer_id: Some(peer),
                error,
                ..
            } => {
                tracing::debug!(%peer, "dial failed: {error}");
                if !self.swarm.is_connected(&peer) {
                    self.release_queued(peer, false);
                }
            }
            SwarmEvent::Behaviour(ev) => self.on_behaviour(ev),
            _ => {}
        }
    }

    fn on_behaviour(&mut self, ev: BehaviourEvent) {
        use request_response::{Event as RR, Message as M};
        match ev {
            BehaviourEvent::Msg(RR::Message { peer, message, .. }) => match message {
                M::Request {
                    request, channel, ..
                } => {
                    let resp = self.core.handle_msg(peer, request);
                    let _ = self.swarm.behaviour_mut().msg.send_response(channel, resp);
                }
                M::Response {
                    request_id,
                    response,
                } => {
                    if let Some(r) = self.pending_msg.remove(&request_id) {
                        let _ = r.send(Ok(response));
                    }
                }
            },
            BehaviourEvent::Msg(RR::OutboundFailure {
                peer,
                request_id,
                error,
                ..
            }) => {
                tracing::debug!(%peer, "msg request failed: {error}");
                if let Some(r) = self.pending_msg.remove(&request_id) {
                    let _ = r.send(Err(anyhow!("delivery to {peer} failed: {error}")));
                }
            }
            BehaviourEvent::Keys(RR::Message { peer, message, .. }) => match message {
                M::Request {
                    request, channel, ..
                } => {
                    let resp = self.core.handle_keys(peer, request);
                    let _ = self.swarm.behaviour_mut().keys.send_response(channel, resp);
                }
                M::Response {
                    request_id,
                    response,
                } => {
                    if let Some(r) = self.pending_keys.remove(&request_id) {
                        let _ = r.send(Ok(response));
                    }
                }
            },
            BehaviourEvent::Keys(RR::OutboundFailure {
                peer,
                request_id,
                error,
                ..
            }) => {
                if let Some(r) = self.pending_keys.remove(&request_id) {
                    let _ = r.send(Err(anyhow!("key claim from {peer} failed: {error}")));
                }
            }
            BehaviourEvent::Mdns(mdns::Event::Discovered(list)) => {
                let mut peers = HashSet::new();
                for (p, a) in list {
                    self.book.entry(p).or_default().insert(a);
                    peers.insert(p);
                }
                for p in peers {
                    self.core.on_discovered(p);
                }
            }
            BehaviourEvent::Identify(identify::Event::Received { peer_id, info, .. }) => {
                let kad_proto = StreamProtocol::new(KAD_PROTOCOL);
                let is_kad = info.protocols.contains(&kad_proto);
                for a in info.listen_addrs {
                    if is_kad {
                        self.swarm
                            .behaviour_mut()
                            .kad
                            .add_address(&peer_id, a.clone());
                    }
                    self.book.entry(peer_id).or_default().insert(a);
                }
            }
            BehaviourEvent::Kad(kad::Event::OutboundQueryProgressed {
                result: kad::QueryResult::GetClosestPeers(Ok(ok)),
                ..
            }) => {
                for p in ok.peers {
                    if !p.addrs.is_empty() {
                        self.book.entry(p.peer_id).or_default().extend(p.addrs);
                        self.core.on_discovered(p.peer_id);
                    }
                }
            }
            BehaviourEvent::Kad(kad::Event::RoutingUpdated {
                peer, addresses, ..
            }) => {
                self.book
                    .entry(peer)
                    .or_default()
                    .extend(addresses.iter().cloned());
            }
            _ => {}
        }
    }
}

#[cfg(all(test, feature = "tor", unix))]
mod tor_dir_tests {
    use super::private_dir;
    use std::os::unix::fs::PermissionsExt;

    #[test]
    fn tor_state_dir_is_owner_only() {
        let tmp = tempfile::tempdir().unwrap();
        let d = tmp.path().join("tor").join("state");
        std::fs::create_dir_all(&d).unwrap();
        std::fs::set_permissions(&d, std::fs::Permissions::from_mode(0o755)).unwrap();
        private_dir(&d).unwrap();
        let mode = std::fs::metadata(&d).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o700);
    }
}
