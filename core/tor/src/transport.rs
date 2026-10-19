//! libp2p [`Transport`] backed by an embedded arti client.
//!
//! Compiled only with `--features tor`. Dial-only unless the **EXPERIMENTAL**
//! `tor-listen` feature is also enabled, in which case `listen_on` with an
//! `/onion3` address hosts a v3 onion service (see [`crate::listen`]).

use crate::{multiaddr_to_target, TorConfig, TorError, TorTarget};
use arti_client::{DataStream, StreamPrefs, TorAddr, TorClient, TorClientConfig};
use futures::future::BoxFuture;
use libp2p::core::transport::{DialOpts, ListenerId, Transport, TransportError, TransportEvent};
use libp2p::Multiaddr;
use std::pin::Pin;
use std::task::{Context, Poll};
use tokio_util::compat::{Compat, TokioAsyncReadCompatExt};
use tor_rtcompat::PreferredRuntime;

#[cfg(feature = "tor-listen")]
use listener::OnionListeners;

/// A Tor stream adapted to `futures::io::{AsyncRead, AsyncWrite}` for libp2p.
pub type TorStream = Compat<DataStream>;

/// libp2p transport over Tor (dial; plus EXPERIMENTAL onion listen with `tor-listen`).
///
/// `Clone` shares the underlying arti client. With `tor-listen`, active onion
/// listeners belong to the instance that called `listen_on`; clones start with
/// none.
pub struct TorTransport {
    client: TorClient<PreferredRuntime>,
    /// Set only when built from a [`TorConfig`] with custom state/cache dirs;
    /// onion listening is refused otherwise (keys must stay in that state dir).
    #[cfg(feature = "tor-listen")]
    listen_cfg: Option<TorConfig>,
    #[cfg(feature = "tor-listen")]
    listeners: OnionListeners,
}

impl Clone for TorTransport {
    fn clone(&self) -> Self {
        Self {
            client: self.client.clone(),
            #[cfg(feature = "tor-listen")]
            listen_cfg: self.listen_cfg.clone(),
            #[cfg(feature = "tor-listen")]
            listeners: OnionListeners::default(),
        }
    }
}

impl TorTransport {
    fn new(client: TorClient<PreferredRuntime>, _cfg: Option<&TorConfig>) -> Self {
        Self {
            client,
            #[cfg(feature = "tor-listen")]
            listen_cfg: _cfg.filter(|c| c.has_custom_dirs()).cloned(),
            #[cfg(feature = "tor-listen")]
            listeners: OnionListeners::default(),
        }
    }

    /// Bootstrap with arti's platform-default state/cache dirs.
    ///
    /// Contacts the Tor network; cold start can take several seconds.
    /// On Android prefer [`Self::bootstrapped_with`] and pass app-owned dirs.
    pub async fn bootstrapped() -> Result<Self, TorError> {
        Self::bootstrapped_with(TorConfig::new()).await
    }

    /// Bootstrap using [`TorConfig`] (custom state/cache dirs for mobile).
    pub async fn bootstrapped_with(config: TorConfig) -> Result<Self, TorError> {
        let client = TorClient::create_bootstrapped(config.build_arti_config()?).await?;
        Ok(Self::new(client, Some(&config)))
    }

    /// Create without waiting for bootstrap; arti bootstraps on first use.
    ///
    /// Must be called inside a Tokio runtime. No network I/O happens here.
    pub fn unbootstrapped_with(config: TorConfig) -> Result<Self, TorError> {
        Self::unbootstrapped_inner(config, arti_client::BootstrapBehavior::OnDemand)
    }

    fn unbootstrapped_inner(
        config: TorConfig,
        behavior: arti_client::BootstrapBehavior,
    ) -> Result<Self, TorError> {
        let client = TorClient::builder()
            .config(config.build_arti_config()?)
            .bootstrap_behavior(behavior)
            .create_unbootstrapped()?;
        Ok(Self::new(client, Some(&config)))
    }

    /// Bootstrap from a raw arti config. Onion listening (`tor-listen`) is not
    /// available on transports built this way; use [`Self::bootstrapped_with`].
    pub async fn with_config(config: TorClientConfig) -> Result<Self, TorError> {
        let client = TorClient::create_bootstrapped(config).await?;
        Ok(Self::new(client, None))
    }

    /// Wrap an already-bootstrapped client (e.g. one shared with other code).
    /// Onion listening (`tor-listen`) is not available on transports built this way.
    pub fn from_client(client: TorClient<PreferredRuntime>) -> Self {
        Self::new(client, None)
    }

    /// Dial a [`TorTarget`] directly (used by tests / non-libp2p callers).
    pub async fn connect_target(&self, target: TorTarget) -> Result<TorStream, TorError> {
        let tor_addr = target_to_tor_addr(&target).map_err(TorError::Unsupported)?;
        let mut prefs = StreamPrefs::new();
        if matches!(target, TorTarget::Onion(..)) {
            prefs.connect_to_onion_services(arti_client::config::BoolOrAuto::Explicit(true));
        }
        let stream = self.client.connect_with_prefs(tor_addr, &prefs).await?;
        Ok(stream.compat())
    }
}

fn target_to_tor_addr(target: &TorTarget) -> Result<TorAddr, Multiaddr> {
    // Build a placeholder multiaddr only for the error path.
    let err_addr = Multiaddr::empty();
    match target {
        TorTarget::Ip(sa) => TorAddr::dangerously_from(*sa).map_err(|_| err_addr),
        TorTarget::Host(h, p) | TorTarget::Onion(h, p) => {
            TorAddr::from((h.as_str(), *p)).map_err(|_| err_addr)
        }
    }
}

impl Transport for TorTransport {
    type Output = TorStream;
    type Error = TorError;
    type ListenerUpgrade = BoxFuture<'static, Result<Self::Output, Self::Error>>;
    type Dial = BoxFuture<'static, Result<Self::Output, Self::Error>>;

    fn listen_on(
        &mut self,
        id: ListenerId,
        addr: Multiaddr,
    ) -> Result<(), TransportError<Self::Error>> {
        #[cfg(feature = "tor-listen")]
        {
            let Some(req) = crate::listen::parse_onion_listen_addr(&addr) else {
                return Err(TransportError::MultiaddrNotSupported(addr));
            };
            let Some(cfg) = self.listen_cfg.as_ref() else {
                return Err(TransportError::Other(TorError::ListenRequiresCustomDirs));
            };
            self.listeners
                .launch(&self.client, cfg, id, req)
                .map_err(TransportError::Other)
        }
        #[cfg(not(feature = "tor-listen"))]
        {
            let _ = id;
            // Inbound onion hosting needs the EXPERIMENTAL `tor-listen` feature.
            Err(TransportError::MultiaddrNotSupported(addr))
        }
    }

    fn remove_listener(&mut self, id: ListenerId) -> bool {
        #[cfg(feature = "tor-listen")]
        {
            self.listeners.remove(id)
        }
        #[cfg(not(feature = "tor-listen"))]
        {
            let _ = id;
            false
        }
    }

    fn dial(
        &mut self,
        addr: Multiaddr,
        _opts: DialOpts,
    ) -> Result<Self::Dial, TransportError<Self::Error>> {
        let Some(target) = multiaddr_to_target(&addr) else {
            return Err(TransportError::MultiaddrNotSupported(addr));
        };
        let tor_addr = match target_to_tor_addr(&target) {
            Ok(a) => a,
            Err(_) => return Err(TransportError::MultiaddrNotSupported(addr)),
        };

        let mut prefs = StreamPrefs::new();
        if matches!(target, TorTarget::Onion(..)) {
            prefs.connect_to_onion_services(arti_client::config::BoolOrAuto::Explicit(true));
        }
        let client = self.client.clone();
        tracing::debug!(%addr, "dialing over tor");
        Ok(Box::pin(async move {
            let stream = client.connect_with_prefs(tor_addr, &prefs).await?;
            Ok(stream.compat())
        }))
    }

    fn poll(
        self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
    ) -> Poll<TransportEvent<Self::ListenerUpgrade, Self::Error>> {
        #[cfg(feature = "tor-listen")]
        {
            self.get_mut().listeners.poll(_cx)
        }
        #[cfg(not(feature = "tor-listen"))]
        {
            Poll::Pending
        }
    }
}

/// EXPERIMENTAL onion-service listener plumbing (`tor-listen`).
#[cfg(feature = "tor-listen")]
mod listener {
    use super::{TorStream, TorTransport};
    use crate::listen::{build_service_config, hsid_to_multiaddr, OnionListenRequest};
    use crate::{TorConfig, TorError};
    use futures::future::BoxFuture;
    use futures::stream::{BoxStream, FuturesUnordered};
    use futures::{FutureExt as _, StreamExt as _};
    use libp2p::core::transport::{ListenerId, TransportEvent};
    use libp2p::Multiaddr;
    use std::collections::VecDeque;
    use std::sync::Arc;
    use std::task::{Context, Poll, Waker};
    use tokio_util::compat::TokioAsyncReadCompatExt;
    use tor_cell::relaycell::msg::{Connected, End, EndReason};
    use tor_hsservice::{RunningOnionService, StreamRequest};
    use tor_proto::client::stream::IncomingStreamRequest;
    use tor_rtcompat::PreferredRuntime;

    type Upgrade = <TorTransport as libp2p::core::Transport>::ListenerUpgrade;
    type Event = TransportEvent<Upgrade, TorError>;

    struct OnionListener {
        id: ListenerId,
        /// Dropping this shuts the onion service down.
        _service: Arc<RunningOnionService>,
        requests: BoxStream<'static, StreamRequest>,
        virtual_port: u16,
        /// Our `/onion3/<own>:<port>`; also used as `send_back_addr` because
        /// onion clients are anonymous (there is no remote address).
        addr: Multiaddr,
    }

    #[derive(Default)]
    pub(super) struct OnionListeners {
        active: Vec<OnionListener>,
        events: VecDeque<Event>,
        rejects: FuturesUnordered<BoxFuture<'static, ()>>,
        waker: Option<Waker>,
    }

    impl OnionListeners {
        pub(super) fn launch(
            &mut self,
            client: &arti_client::TorClient<PreferredRuntime>,
            cfg: &TorConfig,
            id: ListenerId,
            req: OnionListenRequest,
        ) -> Result<(), TorError> {
            if !self.active.is_empty() {
                return Err(TorError::OnionListenerActive);
            }
            let nickname = cfg.onion_nickname_or_default();
            let svc_cfg = build_service_config(nickname)?;
            let port = req.port();
            let (service, rend_requests) = client.launch_onion_service(svc_cfg)?;
            // The identity key is created (or loaded) in <state_dir>/keystore
            // by arti during launch; we only read its *public* HsId.
            let hsid = service
                .onion_address()
                .ok_or(TorError::OnionIdentityMissing)?;
            let addr = hsid_to_multiaddr(&hsid, port).ok_or(TorError::OnionIdentityMissing)?;
            if let OnionListenRequest::Exact { addr: wanted, .. } = &req {
                if *wanted != addr {
                    // `service` drops here, shutting the service down.
                    return Err(TorError::OnionAddressMismatch);
                }
            }
            tracing::info!(
                nickname,
                port,
                "EXPERIMENTAL onion service listener launched"
            );
            self.events.push_back(TransportEvent::NewAddress {
                listener_id: id,
                listen_addr: addr.clone(),
            });
            self.active.push(OnionListener {
                id,
                _service: service,
                requests: tor_hsservice::handle_rend_requests(rend_requests).boxed(),
                virtual_port: port,
                addr,
            });
            if let Some(w) = self.waker.take() {
                w.wake();
            }
            Ok(())
        }

        pub(super) fn remove(&mut self, id: ListenerId) -> bool {
            let Some(pos) = self.active.iter().position(|l| l.id == id) else {
                return false;
            };
            let l = self.active.swap_remove(pos);
            self.events.push_back(TransportEvent::AddressExpired {
                listener_id: id,
                listen_addr: l.addr,
            });
            self.events.push_back(TransportEvent::ListenerClosed {
                listener_id: id,
                reason: Ok(()),
            });
            if let Some(w) = self.waker.take() {
                w.wake();
            }
            true
        }

        pub(super) fn poll(&mut self, cx: &mut Context<'_>) -> Poll<Event> {
            // Drive pending rejections (END cells) to completion.
            while let Poll::Ready(Some(())) = self.rejects.poll_next_unpin(cx) {}

            if let Some(ev) = self.events.pop_front() {
                return Poll::Ready(ev);
            }

            let mut i = 0;
            while i < self.active.len() {
                let l = &mut self.active[i];
                match l.requests.poll_next_unpin(cx) {
                    Poll::Ready(Some(req)) => {
                        if is_begin_for_port(req.request(), l.virtual_port) {
                            let upgrade: Upgrade = async move {
                                let stream = req.accept(Connected::new_empty()).await?;
                                Ok::<TorStream, TorError>(stream.compat())
                            }
                            .boxed();
                            return Poll::Ready(TransportEvent::Incoming {
                                listener_id: l.id,
                                upgrade,
                                local_addr: l.addr.clone(),
                                send_back_addr: l.addr.clone(),
                            });
                        }
                        // Wrong port / not BEGIN: reject like other onion
                        // implementations do (END with DONE), off the hot path.
                        self.rejects.push(
                            async move {
                                let _ = req.reject(End::new_with_reason(EndReason::DONE)).await;
                            }
                            .boxed(),
                        );
                        // Re-poll this same listener.
                    }
                    Poll::Ready(None) => {
                        let l = self.active.swap_remove(i);
                        return Poll::Ready(TransportEvent::ListenerClosed {
                            listener_id: l.id,
                            reason: Ok(()),
                        });
                    }
                    Poll::Pending => i += 1,
                }
            }
            self.waker = Some(cx.waker().clone());
            Poll::Pending
        }
    }

    fn is_begin_for_port(req: &IncomingStreamRequest, port: u16) -> bool {
        matches!(req, IncomingStreamRequest::Begin(b) if b.port() == port)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use libp2p::core::transport::ListenerId;

    /// arti's fs-mistrust refuses group/world-accessible state dirs, so tests
    /// (like apps) must create them private (0700 on Unix).
    fn temp_cfg(tmp: &tempfile::TempDir) -> TorConfig {
        let state = tmp.path().join("state");
        let cache = tmp.path().join("cache");
        for d in [tmp.path(), &state, &cache] {
            std::fs::create_dir_all(d).unwrap();
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt as _;
                std::fs::set_permissions(d, std::fs::Permissions::from_mode(0o700)).unwrap();
            }
        }
        TorConfig::new().state_dir(state).cache_dir(cache)
    }

    /// Offline: Manual bootstrap means arti never touches the network.
    fn offline_transport(cfg: TorConfig) -> TorTransport {
        TorTransport::unbootstrapped_inner(cfg, arti_client::BootstrapBehavior::Manual)
            .expect("unbootstrapped client")
    }

    #[tokio::test]
    async fn listen_rejects_non_onion_addrs() {
        let tmp = tempfile::tempdir().unwrap();
        let mut t = offline_transport(temp_cfg(&tmp));
        let addr: Multiaddr = "/ip4/0.0.0.0/tcp/4001".parse().unwrap();
        assert!(matches!(
            t.listen_on(ListenerId::next(), addr),
            Err(TransportError::MultiaddrNotSupported(_))
        ));
        assert!(!t.remove_listener(ListenerId::next()));
    }

    /// Without `tor-listen`, even `/onion3` listen addrs are unsupported.
    #[cfg(not(feature = "tor-listen"))]
    #[tokio::test]
    async fn onion_listen_needs_feature() {
        let tmp = tempfile::tempdir().unwrap();
        let mut t = offline_transport(temp_cfg(&tmp));
        let addr: Multiaddr =
            "/onion3/2gzyxa5ihm7nsggfxnu52rck2vv4rvmdlkiu3zzui5du4xyclen53wid:4001"
                .parse()
                .unwrap();
        assert!(matches!(
            t.listen_on(ListenerId::next(), addr),
            Err(TransportError::MultiaddrNotSupported(_))
        ));
    }

    #[cfg(feature = "tor-listen")]
    mod listen {
        use super::*;
        use futures::future::poll_fn;
        use std::time::Duration;

        #[tokio::test]
        async fn refuses_without_custom_dirs() {
            let tmp = tempfile::tempdir().unwrap();
            let cfg = temp_cfg(&tmp);
            let client = TorClient::builder()
                .config(cfg.build_arti_config().unwrap())
                .bootstrap_behavior(arti_client::BootstrapBehavior::Manual)
                .create_unbootstrapped()
                .unwrap();
            // from_client: dirs unknown → listening refused.
            let mut t = TorTransport::from_client(client);
            assert!(matches!(
                t.listen_on(ListenerId::next(), crate::onion_listen_addr(4001)),
                Err(TransportError::Other(TorError::ListenRequiresCustomDirs))
            ));
        }

        #[tokio::test]
        async fn invalid_nickname_rejected() {
            let tmp = tempfile::tempdir().unwrap();
            let mut t = offline_transport(temp_cfg(&tmp).onion_nickname("not ok/.."));
            assert!(matches!(
                t.listen_on(ListenerId::next(), crate::onion_listen_addr(4001)),
                Err(TransportError::Other(TorError::InvalidNickname(_)))
            ));
        }

        /// Launch locally (no bootstrap, no network): key goes into
        /// <state_dir>/keystore, NewAddress reports a stable onion3 address,
        /// and an exact listen for a different address is refused.
        #[tokio::test]
        async fn wildcard_listen_reports_onion_addr_and_persists_key() {
            let tmp = tempfile::tempdir().unwrap();
            let cfg = temp_cfg(&tmp).onion_nickname("umbra-test");
            let keystore = cfg.keystore_dir().unwrap();
            let mut t = offline_transport(cfg.clone());
            let id = ListenerId::next();
            t.listen_on(id, crate::onion_listen_addr(4001))
                .expect("launch");

            let ev = tokio::time::timeout(
                Duration::from_secs(10),
                poll_fn(|cx| Pin::new(&mut t).poll(cx)),
            )
            .await
            .expect("event");
            let TransportEvent::NewAddress {
                listener_id,
                listen_addr,
            } = ev
            else {
                panic!("expected NewAddress");
            };
            assert_eq!(listener_id, id);
            assert!(matches!(
                crate::parse_onion_listen_addr(&listen_addr),
                Some(crate::OnionListenRequest::Exact { port: 4001, .. })
            ));

            // Key material only under the configured state dir's keystore.
            let found = walk(&keystore);
            assert!(!found.is_empty(), "no key files under {keystore:?}");
            assert!(found.iter().all(|p| p.starts_with(&keystore)));

            // Removing emits AddressExpired then ListenerClosed.
            assert!(t.remove_listener(id));
            let ev = poll_fn(|cx| Pin::new(&mut t).poll(cx)).await;
            assert!(matches!(ev, TransportEvent::AddressExpired { .. }));
            let ev = poll_fn(|cx| Pin::new(&mut t).poll(cx)).await;
            assert!(matches!(ev, TransportEvent::ListenerClosed { .. }));

            // "Restart": copy ONLY the state dir to a fresh location (a copy
            // does not carry arti's per-nickname lock, which an offline arti
            // 0.35 service never releases in-process) and relaunch. Same
            // address => identity lives entirely in state_dir.
            let tmp2 = tempfile::tempdir().unwrap();
            let cfg2 = temp_cfg(&tmp2).onion_nickname("umbra-test");
            copy_dir(
                cfg.state_dir_path().unwrap(),
                cfg2.state_dir_path().unwrap(),
            );
            let mut t2 = offline_transport(cfg2.clone());
            t2.listen_on(ListenerId::next(), listen_addr.clone())
                .expect("same identity from copied state dir");
            // Second listener on the same transport is refused clearly.
            assert!(matches!(
                t2.listen_on(ListenerId::next(), crate::onion_listen_addr(4002)),
                Err(TransportError::Other(TorError::OnionListenerActive))
            ));

            // A different identity (fresh state) refuses our exact address.
            let tmp3 = tempfile::tempdir().unwrap();
            let mut t3 = offline_transport(temp_cfg(&tmp3).onion_nickname("umbra-test"));
            assert!(matches!(
                t3.listen_on(ListenerId::next(), listen_addr),
                Err(TransportError::Other(TorError::OnionAddressMismatch))
            ));
        }

        fn copy_dir(from: &std::path::Path, to: &std::path::Path) {
            std::fs::create_dir_all(to).unwrap();
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt as _;
                std::fs::set_permissions(to, std::fs::Permissions::from_mode(0o700)).unwrap();
            }
            for e in std::fs::read_dir(from).unwrap().flatten() {
                let (src, dst) = (e.path(), to.join(e.file_name()));
                if src.is_dir() {
                    copy_dir(&src, &dst);
                } else {
                    std::fs::copy(&src, &dst).unwrap();
                }
            }
        }

        fn walk(dir: &std::path::Path) -> Vec<std::path::PathBuf> {
            let mut out = Vec::new();
            if let Ok(rd) = std::fs::read_dir(dir) {
                for e in rd.flatten() {
                    let p = e.path();
                    if p.is_dir() {
                        out.extend(walk(&p));
                    } else {
                        out.push(p);
                    }
                }
            }
            out
        }
    }
}
