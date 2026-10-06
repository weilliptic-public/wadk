pub(crate) const DEFAULT_CONCURRENCY: usize = 64;
#[cfg(feature = "local")]
pub(crate) const SENTINEL_HOST: &str = "https://sentinel-local.weilliptic.ai";
#[cfg(all(feature = "staging", not(feature = "local")))]
pub(crate) const SENTINEL_HOST: &str = "https://sentinel-dev.weilliptic.ai";
#[cfg(not(any(feature = "local", feature = "staging")))]
pub(crate) const SENTINEL_HOST: &str = "https://sentinel.weilliptic.ai";
pub(crate) const DEFAULT_CONCURRENCY: usize = 64;
