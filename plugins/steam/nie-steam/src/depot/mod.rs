// SPDX-License-Identifier: Apache-2.0
//! Steam depot acquisition, absorbed from `nie-re` (originally `nie-steam`, the Rust port
//! of the reference C# Steam layer) over `steamroom`, which owns the Steam protocol (CM, auth,
//! manifests, chunks, CDN).
//!
//! [`planning`] is platform-neutral and always available (also on `wasm32`); the CM client
//! (session, downloader, transfer, per-account token cache) needs feature `depot`.

/// Platform-neutral refresh-token lifecycle (JWT claims, renewal wire format, rotation).
pub mod auth;
/// Platform-neutral depot selection and manifest collision planning.
pub mod planning;

/// Pure depot selection over PICS KeyValues.
#[cfg(feature = "depot")]
pub mod depot_resolver;
/// Depot download orchestration.
#[cfg(feature = "depot")]
pub mod downloader;
/// Download options and progress types.
#[cfg(feature = "depot")]
pub mod options;
/// Authenticated CM session, PICS and depot keys.
#[cfg(feature = "depot")]
pub mod session;
/// Pluggable refresh-token stores (`NIE_STEAM_TOKEN_STORE`).
#[cfg(feature = "depot")]
pub mod token_backend;
/// Per-account refresh-token and guard-data cache of the CM client.
#[cfg(feature = "depot")]
pub mod token_store;
/// Chunk cache, rate limiting and transfer counters.
#[cfg(feature = "depot")]
pub mod transfer;
