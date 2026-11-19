//! ECDSA P-256 keys for rcgen, backed by graviola (Rust and assembly, no ring, no aws-lc).

use anyhow::{Result, anyhow, bail};
use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use graviola::hashing::Sha256;
use graviola::signing::ecdsa::{P256, SigningKey};
use rcgen::{Error as RcgenError, PKCS_ECDSA_P256_SHA256, PublicKeyData, SignatureAlgorithm};

/// Length of an uncompressed P-256 point: `04 || x || y`.
const POINT_LEN: usize = 65;

/// A P-256 signing key that rcgen can sign certificates and requests with.
pub struct P256Key {
    key: SigningKey<P256>,
    public: Vec<u8>,
}

impl P256Key {
    /// A fresh random key.
    pub fn generate() -> Result<Self> {
        if !graviola::util::is_cpu_supported() {
            bail!("graviola needs the adx, aes, avx, avx2, bmi1 and pclmulqdq CPU features");
        }
        let key = SigningKey::<P256>::generate().map_err(|e| anyhow!("p-256 key: {e:?}"))?;
        let mut spki = [0u8; 128];
        let spki = key
            .to_spki_der(&mut spki)
            .map_err(|e| anyhow!("p-256 spki: {e:?}"))?;
        let public = spki
            .get(spki.len().saturating_sub(POINT_LEN)..)
            .filter(|point| point.len() == POINT_LEN && point[0] == 4)
            .ok_or_else(|| anyhow!("p-256 spki without an uncompressed point"))?
            .to_vec();
        Ok(Self { key, public })
    }

    /// The private key as a PKCS#8 PEM (`BEGIN PRIVATE KEY`).
    pub fn private_pem(&self) -> Result<String> {
        let mut der = [0u8; 256];
        let der = self
            .key
            .to_pkcs8_der(&mut der)
            .map_err(|e| anyhow!("p-256 pkcs8: {e:?}"))?;
        let body = STANDARD.encode(der);
        let mut pem = String::from("-----BEGIN PRIVATE KEY-----\n");
        for line in body.as_bytes().chunks(64) {
            pem.push_str(std::str::from_utf8(line)?);
            pem.push('\n');
        }
        pem.push_str("-----END PRIVATE KEY-----\n");
        Ok(pem)
    }
}

impl PublicKeyData for P256Key {
    fn der_bytes(&self) -> &[u8] {
        &self.public
    }

    fn algorithm(&self) -> &'static SignatureAlgorithm {
        &PKCS_ECDSA_P256_SHA256
    }
}

impl rcgen::SigningKey for P256Key {
    fn sign(&self, msg: &[u8]) -> Result<Vec<u8>, RcgenError> {
        let mut out = [0u8; 80];
        self.key
            .sign_asn1::<Sha256>(&[msg], &mut out)
            .map(<[u8]>::to_vec)
            .map_err(|_| RcgenError::RemoteKeyError)
    }
}
