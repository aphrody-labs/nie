//! Thin synchronous Zukan client: blocking HTTP with host pacing and bounded retries.
//! Domain cache paths stay here; the network side is one `reqwest` blocking client.

use anyhow::{Context, Result, bail};
use reqwest::blocking::Client;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant};
use tracing::debug;

/// Official Zukan base URL.
pub const BASE_URL: &str = "https://zukan.inazuma.jp";

/// Identifying User-Agent sent with every request.
pub const USER_AGENT: &str = "RoseGriffon-Zukan/1.0 (+https://rosegriffon.fr)";

/// Minimum delay between requests to the same host.
pub const RATE_LIMIT_MS: u64 = 350;

/// Maximum attempts for transient HTTP failures.
pub const MAX_RETRIES: u32 = 3;

/// Synchronous Web client with a domain-owned disk cache.
pub struct ZukanClient {
    http: Client,
    last_request: Mutex<Option<Instant>>,
    cache_root: PathBuf,
}

impl ZukanClient {
    /// Creates a client using the supplied domain cache directory.
    pub fn new(cache_root: PathBuf) -> Result<Self> {
        let http = Client::builder()
            .user_agent(USER_AGENT)
            .timeout(Duration::from_secs(30))
            .build()
            .context("initializing the HTTP client")?;
        Ok(Self {
            http,
            last_request: Mutex::new(None),
            cache_root,
        })
    }

    /// Returns the domain cache path for a resource.
    ///
    /// `cache_key` is an opaque identifier such as `page_1` or `c01000010`.
    #[must_use]
    pub fn cache_path(&self, lang: &str, route: &str, cache_key: &str) -> PathBuf {
        self.cache_root
            .join(lang)
            .join(route)
            .join(format!("{cache_key}.html"))
    }

    /// Reads an existing cache entry or fetches it from the network.
    /// Successful acquisitions are stored under the supplied cache path.
    pub fn get_cached(&self, url: &str, cache_path: &Path) -> Result<String> {
        // Read the domain cache first
        if cache_path.exists() {
            debug!(path = %cache_path.display(), "cache hit");
            return std::fs::read_to_string(cache_path).context("reading the domain cache");
        }

        let content = self.fetch_with_retry(url)?;

        // Store the successful domain response
        if let Some(parent) = cache_path.parent() {
            std::fs::create_dir_all(parent).context("creating the domain cache directory")?;
        }
        std::fs::write(cache_path, &content).context("writing the domain cache")?;
        debug!(path = %cache_path.display(), "cache miss: saved");

        Ok(content)
    }

    /// Waits until `RATE_LIMIT_MS` has elapsed since the previous request.
    fn pace(&self) {
        let mut last = self
            .last_request
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(previous) = *last {
            let wait = Duration::from_millis(RATE_LIMIT_MS).saturating_sub(previous.elapsed());
            if !wait.is_zero() {
                std::thread::sleep(wait);
            }
        }
        *last = Some(Instant::now());
    }

    /// GET with pacing; retries transient failures (network error, 429, 5xx) with backoff.
    fn fetch_with_retry(&self, url: &str) -> Result<String> {
        let mut last_error = String::new();
        for attempt in 0..MAX_RETRIES {
            if attempt > 0 {
                std::thread::sleep(Duration::from_millis(RATE_LIMIT_MS << attempt));
            }
            self.pace();
            match self.http.get(url).send() {
                Ok(response) => {
                    let status = response.status();
                    if status.is_success() {
                        return response.text().context("reading the response body");
                    }
                    last_error = format!("HTTP {status}");
                    if !(status.as_u16() == 429 || status.is_server_error()) {
                        break;
                    }
                }
                Err(error) => last_error = error.to_string(),
            }
        }
        bail!("GET {url} failed: {last_error}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cache_path_structure() {
        let client = ZukanClient::new(PathBuf::from("/tmp/test-cache")).unwrap();
        let p = client.cache_path("ja", "chara_list", "page_1");
        assert_eq!(
            p,
            PathBuf::from("/tmp/test-cache/ja/chara_list/page_1.html")
        );
    }

    #[test]
    fn cache_path_fr() {
        let client = ZukanClient::new(PathBuf::from("/tmp/test-cache")).unwrap();
        let p = client.cache_path("fr", "chara_param", "c01000010");
        assert_eq!(
            p,
            PathBuf::from("/tmp/test-cache/fr/chara_param/c01000010.html")
        );
    }
}
