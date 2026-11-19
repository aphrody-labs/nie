//! The one TLS crypto provider of the nie graph.
//!
//! Every rustls consumer (reqwest, ureq, tokio-postgres, pingora, quinn) takes its primitives from
//! [`rustls-graviola`](rustls_graviola): Rust plus assembly, no C and no OpenSSL. `deny.toml` bans
//! `aws-lc-sys`, `aws-lc-rs`, `ring`, `openssl-sys` and `libssh2-sys`, so a crate that brings
//! another provider back fails CI. Binaries call [`install`] once at startup; code that builds its
//! own `ClientConfig` or `ServerConfig` uses [`provider`].

use std::sync::Arc;

use rustls::crypto::CryptoProvider;

/// The provider, shared: `ClientConfig::builder_with_provider(nie_tls::provider())`.
#[must_use]
pub fn provider() -> Arc<CryptoProvider> {
    match CryptoProvider::get_default() {
        Some(installed) => installed.clone(),
        None => Arc::new(rustls_graviola::default_provider()),
    }
}

/// Install the provider as the process default. Idempotent: a provider already installed (by this
/// call or another) is kept.
pub fn install() {
    if CryptoProvider::get_default().is_none() {
        let _ = rustls_graviola::default_provider().install_default();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn install_is_idempotent_and_provider_has_suites() {
        install();
        install();
        let provider = provider();
        assert!(!provider.cipher_suites.is_empty());
        assert!(CryptoProvider::get_default().is_some());
    }

    #[test]
    fn client_config_builds_on_the_provider() {
        let config = rustls::ClientConfig::builder_with_provider(provider())
            .with_safe_default_protocol_versions()
            .expect("tls versions")
            .with_root_certificates(rustls::RootCertStore::empty())
            .with_no_client_auth();
        assert!(config.alpn_protocols.is_empty());
    }
}
