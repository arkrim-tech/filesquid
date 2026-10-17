//! The crypto-backend trait (C2PA-S-006).
//!
//! C2PA logic in this crate never calls a cryptography crate directly: hashes
//! and signature checks go through a [`CryptoBackend`]. The default
//! `rust-crypto` feature provides [`RustCryptoBackend`] on established,
//! audited-in-the-open RustCrypto crates. A host MAY supply its own backend,
//! for example one that uses WebCrypto in a browser or a hardware module.
//! Primitives are never hand-rolled.

use std::future::Future;

#[cfg(feature = "rust-crypto")]
mod rust_crypto;

#[cfg(feature = "rust-crypto")]
pub use rust_crypto::RustCryptoBackend;

/// Hash algorithms C2PA uses for bindings and hashed URIs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HashAlgorithm {
    /// SHA-256 (the FileSquid default digest).
    Sha256,
    /// SHA-384.
    Sha384,
    /// SHA-512.
    Sha512,
}

impl HashAlgorithm {
    /// Parse the `alg` name used in C2PA assertions (`sha256`, `sha384`, `sha512`).
    pub fn from_c2pa_name(name: &str) -> Option<Self> {
        match name {
            "sha256" => Some(Self::Sha256),
            "sha384" => Some(Self::Sha384),
            "sha512" => Some(Self::Sha512),
            _ => None,
        }
    }

    /// The C2PA `alg` name.
    pub fn c2pa_name(self) -> &'static str {
        match self {
            Self::Sha256 => "sha256",
            Self::Sha384 => "sha384",
            Self::Sha512 => "sha512",
        }
    }

    /// Digest length in bytes.
    pub fn output_len(self) -> usize {
        match self {
            Self::Sha256 => 32,
            Self::Sha384 => 48,
            Self::Sha512 => 64,
        }
    }
}

/// Claim-signature algorithms allowed by the C2PA specification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SigningAlgorithm {
    /// ECDSA on P-256 with SHA-256.
    Es256,
    /// ECDSA on P-384 with SHA-384.
    Es384,
    /// ECDSA on P-521 with SHA-512.
    Es512,
    /// RSASSA-PSS with SHA-256.
    Ps256,
    /// RSASSA-PSS with SHA-384.
    Ps384,
    /// RSASSA-PSS with SHA-512.
    Ps512,
    /// EdDSA on Ed25519.
    Ed25519,
}

/// (algorithm, COSE identifier, C2PA name) for every [`SigningAlgorithm`].
const SIGNING_TABLE: [(SigningAlgorithm, i64, &str); 7] = [
    (SigningAlgorithm::Es256, -7, "es256"),
    (SigningAlgorithm::Es384, -35, "es384"),
    (SigningAlgorithm::Es512, -36, "es512"),
    (SigningAlgorithm::Ps256, -37, "ps256"),
    (SigningAlgorithm::Ps384, -38, "ps384"),
    (SigningAlgorithm::Ps512, -39, "ps512"),
    (SigningAlgorithm::Ed25519, -8, "ed25519"),
];

impl SigningAlgorithm {
    /// Map a COSE `alg` header value (RFC 9053 / IANA registry) to an algorithm.
    pub fn from_cose_id(id: i64) -> Option<Self> {
        SIGNING_TABLE
            .iter()
            .find(|(_, cose, _)| *cose == id)
            .map(|(alg, _, _)| *alg)
    }

    /// The COSE `alg` header value.
    pub fn cose_id(self) -> i64 {
        self.entry().1
    }

    /// The lowercase name the C2PA specification uses.
    pub fn c2pa_name(self) -> &'static str {
        self.entry().2
    }

    fn entry(self) -> (SigningAlgorithm, i64, &'static str) {
        // Every variant has exactly one row, so the lookup cannot miss.
        SIGNING_TABLE
            .into_iter()
            .find(|(alg, _, _)| *alg == self)
            .unwrap_or(SIGNING_TABLE[0])
    }
}

/// Errors from a crypto backend.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CryptoError {
    /// The backend does not implement this signature algorithm.
    #[error("signature algorithm {0:?} is not supported by this backend")]
    UnsupportedAlgorithm(SigningAlgorithm),
    /// The backend does not implement this hash algorithm.
    #[error("hash algorithm {0:?} is not supported by this backend")]
    UnsupportedHash(HashAlgorithm),
    /// The public key is malformed or does not match the algorithm.
    #[error("invalid public key: {0}")]
    InvalidKey(String),
    /// The signature bytes are malformed for the algorithm.
    #[error("invalid signature encoding: {0}")]
    InvalidSignature(String),
    /// The signature is well-formed but does not verify.
    #[error("signature verification failed")]
    VerificationFailed,
    /// Any other backend failure.
    #[error("crypto backend error: {0}")]
    Backend(String),
}

/// A running hash computation, fed in chunks as bytes arrive from a
/// [`crate::io::ByteSource`].
pub trait IncrementalHasher: Send {
    /// The algorithm being computed.
    fn algorithm(&self) -> HashAlgorithm;

    /// Feed more bytes.
    fn update(&mut self, data: &[u8]);

    /// Finish and return the digest.
    fn finalize(self: Box<Self>) -> Vec<u8>;
}

/// Every cryptographic primitive the engine uses.
pub trait CryptoBackend {
    /// Start an incremental hash.
    fn incremental_hasher(
        &self,
        algorithm: HashAlgorithm,
    ) -> Result<Box<dyn IncrementalHasher>, CryptoError>;

    /// Hash `data` in one call.
    ///
    /// The default feeds [`CryptoBackend::incremental_hasher`]; a backend MAY
    /// override it, for example with WebCrypto `digest`.
    fn digest(
        &self,
        algorithm: HashAlgorithm,
        data: &[u8],
    ) -> impl Future<Output = Result<Vec<u8>, CryptoError>> {
        async move {
            let mut hasher = self.incremental_hasher(algorithm)?;
            hasher.update(data);
            Ok(hasher.finalize())
        }
    }

    /// Verify `signature` over `message` with the key in
    /// `public_key_spki_der` (a DER `SubjectPublicKeyInfo`, as found in the
    /// signing certificate).
    ///
    /// ECDSA signatures use the fixed-size `r || s` form COSE specifies.
    /// Returns `Ok(())` only for a valid signature.
    fn verify_signature(
        &self,
        algorithm: SigningAlgorithm,
        public_key_spki_der: &[u8],
        message: &[u8],
        signature: &[u8],
    ) -> impl Future<Output = Result<(), CryptoError>>;
}
