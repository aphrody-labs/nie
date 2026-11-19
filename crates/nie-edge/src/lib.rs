//! `nie-edge`: the edge proxy of nie.aphrody.app, built on the pingora crates.
//!
//! It routes between `nie serve` (JSON API and wiki) and `nie-model-serve` (decoded media),
//! never exposes the VFS bridge or the game's raw bytes, limits the decode-heavy routes, caches
//! immutable answers, probes both upstreams and terminates Cloudflare origin TLS with mutual TLS
//! (Authenticated Origin Pulls). Pingora runs on unix only; the configuration and the routing
//! table build everywhere.

#![forbid(unsafe_code)]

pub mod config;
pub mod route;

#[cfg(unix)]
pub mod cache;
#[cfg(unix)]
pub mod limits;
#[cfg(unix)]
pub mod proxy;
#[cfg(unix)]
pub mod reload;
#[cfg(unix)]
pub mod serve;
