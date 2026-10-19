//! Dial-only libp2p [`Transport`] backed by an embedded arti client.
//!
//! Compiled only with `--features tor`.

use crate::{multiaddr_to_target, TorConfig, TorError, TorTarget};
use arti_client::{DataStream, StreamPrefs, TorAddr, TorClient, TorClientConfig};
use futures::future::BoxFuture;
use libp2p::core::transport::{DialOpts, ListenerId, Transport, TransportError, TransportEvent};
use libp2p::Multiaddr;
use std::pin::Pin;
use std::task::{Context, Poll};
use tokio_util::compat::{Compat, TokioAsyncReadCompatExt};
use tor_rtcompat::PreferredRuntime;

/// A Tor stream adapted to `futures::io::{AsyncRead, AsyncWrite}` for libp2p.
pub type TorStream = Compat<DataStream>;

/// Dial-only libp2p transport over Tor.
#[derive(Clone)]
pub struct TorTransport {
    client: TorClient<PreferredRuntime>,
}

impl TorTransport {
    /// Bootstrap with arti's platform-default state/cache dirs.
    ///
    /// Contacts the Tor network; cold start can take several seconds.
    /// On Android prefer [`Self::bootstrapped_with`] and pass app-owned dirs.
    pub async fn bootstrapped() -> Result<Self, TorError> {
        Self::bootstrapped_with(TorConfig::new()).await
    }

    /// Bootstrap using [`TorConfig`] (custom state/cache dirs for mobile).
    pub async fn bootstrapped_with(config: TorConfig) -> Result<Self, TorError> {
        Self::with_config(config.build_arti_config()?).await
    }

    pub async fn with_config(config: TorClientConfig) -> Result<Self, TorError> {
        let client = TorClient::create_bootstrapped(config).await?;
        Ok(Self { client })
    }

    /// Wrap an already-bootstrapped client (e.g. one shared with other code).
    pub fn from_client(client: TorClient<PreferredRuntime>) -> Self {
        Self { client }
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
    type ListenerUpgrade = futures::future::Pending<Result<Self::Output, Self::Error>>;
    type Dial = BoxFuture<'static, Result<Self::Output, Self::Error>>;

    fn listen_on(
        &mut self,
        _id: ListenerId,
        addr: Multiaddr,
    ) -> Result<(), TransportError<Self::Error>> {
        // Onion-service hosting is future work (see docs/tor.md).
        Err(TransportError::MultiaddrNotSupported(addr))
    }

    fn remove_listener(&mut self, _id: ListenerId) -> bool {
        false
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
        Poll::Pending
    }
}
