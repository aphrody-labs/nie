//! Response cache on pingora-cache: memory storage, LRU eviction, a cache lock that collapses
//! concurrent misses of one key into a single upstream decode, and a predictor that remembers
//! uncacheable keys. Only `200` answers marked `Cache-Control: immutable` are stored.

use std::time::Duration;

use pingora_cache::cache_control::CacheControl;
use pingora_cache::eviction::simple_lru::Manager;
use pingora_cache::filters::resp_cacheable;
use pingora_cache::lock::{CacheKeyLockImpl, CacheLock};
use pingora_cache::predictor::Predictor;
use pingora_cache::{CacheMetaDefaults, MemCache, NoCacheReason, RespCacheable};
use pingora_http::ResponseHeader;

use crate::config::Cache as CacheConfig;

/// The process-wide cache objects (pingora wants `'static` handles).
pub struct CacheBackend {
    /// Object storage.
    pub storage: &'static MemCache,
    /// LRU eviction.
    pub eviction: &'static Manager,
    /// Uncacheable-key memory.
    pub predictor: &'static Predictor<32>,
    /// Miss collapsing.
    pub lock: &'static CacheKeyLockImpl,
    /// Largest cached object, bytes.
    pub max_object: usize,
}

const MIB: usize = 1024 * 1024;

impl CacheBackend {
    /// Allocate the cache for the lifetime of the process.
    #[must_use]
    pub fn leak(config: &CacheConfig) -> &'static Self {
        let storage: &'static MemCache = Box::leak(Box::new(MemCache::new()));
        let eviction: &'static Manager =
            Box::leak(Box::new(Manager::new(config.capacity_mb.max(1) * MIB)));
        let predictor: &'static Predictor<32> = Box::leak(Box::new(Predictor::new(512, None)));
        let lock: &'static CacheKeyLockImpl =
            Box::leak(CacheLock::new_boxed(Duration::from_secs(30)));
        Box::leak(Box::new(Self {
            storage,
            eviction,
            predictor,
            lock,
            max_object: config.max_object_mb.max(1) * MIB,
        }))
    }
}

/// Freshness comes from `max-age` only: no status gets a default lifetime.
static DEFAULTS: CacheMetaDefaults = CacheMetaDefaults::new(|_| None, 0, 0);

/// Whether an upstream answer may be stored: `200` with `Cache-Control: immutable`.
#[must_use]
pub fn is_immutable(resp: &ResponseHeader) -> bool {
    resp.status.as_u16() == 200
        && CacheControl::from_resp_headers(resp).is_some_and(|cc| cc.has_key("immutable"))
}

/// The cacheability decision of an upstream answer.
#[must_use]
pub fn decide(resp: &ResponseHeader) -> RespCacheable {
    if !is_immutable(resp) {
        return RespCacheable::Uncacheable(NoCacheReason::OriginNotCache);
    }
    let cc = CacheControl::from_resp_headers(resp);
    resp_cacheable(cc.as_ref(), resp.clone(), false, &DEFAULTS)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn resp(status: u16, cache_control: Option<&str>) -> ResponseHeader {
        let mut r = ResponseHeader::build(status, None).unwrap();
        if let Some(v) = cache_control {
            r.insert_header("Cache-Control", v).unwrap();
        }
        r
    }

    #[test]
    fn only_immutable_200_is_cacheable() {
        let ok = resp(200, Some("public, max-age=31536000, immutable"));
        assert!(decide(&ok).is_cacheable());
        assert!(!decide(&resp(200, Some("public, max-age=300"))).is_cacheable());
        assert!(!decide(&resp(200, None)).is_cacheable());
        assert!(!decide(&resp(404, Some("max-age=60, immutable"))).is_cacheable());
        assert!(!decide(&resp(200, Some("no-store, immutable"))).is_cacheable());
        assert!(!decide(&resp(200, Some("immutable"))).is_cacheable());
    }
}
