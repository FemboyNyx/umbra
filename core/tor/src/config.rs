//! Arti state/cache directory configuration.
//!
//! On desktop, arti's defaults (`${ARTI_LOCAL_DATA}`, `${ARTI_CACHE}`) are fine.
//! On Android the app must supply writable dirs (no XDG; scoped storage), so
//! Elite/Flutter should call [`TorConfig::state_dir`] / [`TorConfig::cache_dir`]
//! before bootstrapping [`crate::TorTransport`].

use std::path::{Path, PathBuf};

/// Optional overrides for arti's on-disk state and cache.
///
/// Both dirs should be set together when customizing (e.g. Android app
/// `filesDir` + cache dir). Leaving both `None` keeps arti's platform defaults.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TorConfig {
    state_dir: Option<PathBuf>,
    cache_dir: Option<PathBuf>,
    /// EXPERIMENTAL (`tor-listen`): onion service nickname (keystore namespace).
    #[cfg(feature = "tor-listen")]
    onion_nickname: Option<String>,
}

/// Default onion service nickname when none is configured (`tor-listen`).
#[cfg(feature = "tor-listen")]
pub const DEFAULT_ONION_NICKNAME: &str = "umbra";

impl TorConfig {
    /// Empty config: arti platform defaults.
    pub fn new() -> Self {
        Self::default()
    }

    /// Directory for durable Tor state (keys, guards, …).
    pub fn state_dir(mut self, path: impl Into<PathBuf>) -> Self {
        self.state_dir = Some(path.into());
        self
    }

    /// Directory for Tor directory cache (consensus, descriptors, …).
    pub fn cache_dir(mut self, path: impl Into<PathBuf>) -> Self {
        self.cache_dir = Some(path.into());
        self
    }

    pub fn state_dir_path(&self) -> Option<&Path> {
        self.state_dir.as_deref()
    }

    pub fn cache_dir_path(&self) -> Option<&Path> {
        self.cache_dir.as_deref()
    }

    /// EXPERIMENTAL (`tor-listen`): nickname of the hosted onion service.
    ///
    /// The nickname selects the identity key in the keystore under
    /// `<state_dir>/keystore/`, so keep it stable to keep the same `.onion`
    /// address across restarts. Validated (arti `HsNickname` rules: lowercase
    /// ASCII alphanumerics, `_`, `-`, not starting with `-`) when the listener
    /// launches. Defaults to [`DEFAULT_ONION_NICKNAME`].
    #[cfg(feature = "tor-listen")]
    pub fn onion_nickname(mut self, nickname: impl Into<String>) -> Self {
        self.onion_nickname = Some(nickname.into());
        self
    }

    /// Configured onion nickname, or [`DEFAULT_ONION_NICKNAME`].
    #[cfg(feature = "tor-listen")]
    pub fn onion_nickname_or_default(&self) -> &str {
        self.onion_nickname
            .as_deref()
            .unwrap_or(DEFAULT_ONION_NICKNAME)
    }

    /// Where arti's native keystore lives (`<state_dir>/keystore`), if a custom
    /// state dir is set. Onion service identity keys are written **only** here.
    pub fn keystore_dir(&self) -> Option<PathBuf> {
        self.state_dir.as_ref().map(|s| s.join("keystore"))
    }

    /// `true` when at least one custom directory was set.
    pub fn has_custom_dirs(&self) -> bool {
        self.state_dir.is_some() || self.cache_dir.is_some()
    }

    /// Build an [`arti_client::TorClientConfig`] from this spike config.
    ///
    /// - Both dirs unset → `TorClientConfig::default()`.
    /// - Both dirs set → `TorClientConfigBuilder::from_directories`.
    /// - Only one set → [`crate::TorError::IncompleteDirs`].
    #[cfg(feature = "tor")]
    pub fn build_arti_config(&self) -> Result<arti_client::TorClientConfig, crate::TorError> {
        use arti_client::config::{TorClientConfig, TorClientConfigBuilder};

        match (&self.state_dir, &self.cache_dir) {
            (None, None) => Ok(TorClientConfig::default()),
            (Some(state), Some(cache)) => {
                Ok(TorClientConfigBuilder::from_directories(state, cache).build()?)
            }
            (Some(_), None) | (None, Some(_)) => Err(crate::TorError::IncompleteDirs),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builder_stores_paths() {
        let cfg = TorConfig::new()
            .state_dir("/tmp/umbra-tor-state")
            .cache_dir("/tmp/umbra-tor-cache");
        assert_eq!(
            cfg.state_dir_path().map(Path::to_path_buf),
            Some(PathBuf::from("/tmp/umbra-tor-state"))
        );
        assert_eq!(
            cfg.cache_dir_path().map(Path::to_path_buf),
            Some(PathBuf::from("/tmp/umbra-tor-cache"))
        );
        assert!(cfg.has_custom_dirs());
        assert!(!TorConfig::new().has_custom_dirs());
    }

    #[cfg(feature = "tor")]
    #[test]
    fn custom_dirs_build_arti_config() {
        let tmp = tempfile::tempdir().unwrap();
        let state = tmp.path().join("state");
        let cache = tmp.path().join("cache");
        std::fs::create_dir_all(&state).unwrap();
        std::fs::create_dir_all(&cache).unwrap();
        let cfg = TorConfig::new().state_dir(&state).cache_dir(&cache);
        cfg.build_arti_config()
            .expect("arti config from custom dirs");
    }

    #[cfg(feature = "tor")]
    #[test]
    fn incomplete_dirs_error() {
        let cfg = TorConfig::new().state_dir("/tmp/only-state");
        assert!(matches!(
            cfg.build_arti_config(),
            Err(crate::TorError::IncompleteDirs)
        ));
    }

    #[cfg(feature = "tor")]
    #[test]
    fn default_dirs_build_arti_config() {
        TorConfig::new()
            .build_arti_config()
            .expect("default arti config");
    }
}
