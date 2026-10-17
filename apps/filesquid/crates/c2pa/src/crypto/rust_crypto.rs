//! The default crypto backend, on RustCrypto crates (feature `rust-crypto`).
//!
//! | Primitive | Crate |
//! |---|---|
//! | SHA-256/384/512 | `sha2` |
//! | ECDSA P-256, P-384, P-521 | `p256`, `p384`, `p521` (`ecdsa` 0.17) |
//! | Ed25519 | `ed25519-dalek` |
//!
//! ECDSA takes the curve from the key and the hash from the algorithm, so
//! ES256, ES384 and ES512 each accept P-256, P-384 and P-521 keys, as C2PA 2.4
//! section 13.2 requires.
//!
//! RSA-PSS (`rsa`), X.509 (`x509-cert`) and CMS / RFC 3161 (`cms`) will be
//! added with certificate and timestamp validation in CP-0.4; none of them
//! is a dependency yet. Until then PS256, PS384 and PS512 return
//! [`CryptoError::UnsupportedAlgorithm`]. This is the only module of the
//! library that names a cryptography crate.

use ed25519_dalek::pkcs8::DecodePublicKey as _;

use super::{CryptoBackend, CryptoError, HashAlgorithm, IncrementalHasher, SigningAlgorithm};

/// The pure-Rust backend. Builds for `wasm32-unknown-unknown` and needs no
/// random number generator (it only verifies).
#[derive(Debug, Clone, Copy, Default)]
pub struct RustCryptoBackend;

/// A `sha2` hasher behind [`IncrementalHasher`].
struct Sha2Hasher<D> {
    /// Algorithm reported to callers.
    algorithm: HashAlgorithm,
    /// The running state.
    state: D,
}

impl<D: sha2::Digest + Send + 'static> IncrementalHasher for Sha2Hasher<D> {
    fn algorithm(&self) -> HashAlgorithm {
        self.algorithm
    }

    fn update(&mut self, data: &[u8]) {
        self.state.update(data);
    }

    fn finalize(self: Box<Self>) -> Vec<u8> {
        self.state.finalize().to_vec()
    }
}

/// Box a fresh `D` hasher for `algorithm`.
fn boxed<D: sha2::Digest + Send + 'static>(algorithm: HashAlgorithm) -> Box<dyn IncrementalHasher> {
    Box::new(Sha2Hasher {
        algorithm,
        state: D::new(),
    })
}

impl CryptoBackend for RustCryptoBackend {
    fn incremental_hasher(
        &self,
        algorithm: HashAlgorithm,
    ) -> Result<Box<dyn IncrementalHasher>, CryptoError> {
        Ok(match algorithm {
            HashAlgorithm::Sha256 => boxed::<sha2::Sha256>(algorithm),
            HashAlgorithm::Sha384 => boxed::<sha2::Sha384>(algorithm),
            HashAlgorithm::Sha512 => boxed::<sha2::Sha512>(algorithm),
        })
    }

    async fn verify_signature(
        &self,
        algorithm: SigningAlgorithm,
        public_key_spki_der: &[u8],
        message: &[u8],
        signature: &[u8],
    ) -> Result<(), CryptoError> {
        let (key, msg, sig) = (public_key_spki_der, message, signature);
        match algorithm {
            SigningAlgorithm::Es256 | SigningAlgorithm::Es384 | SigningAlgorithm::Es512 => {
                verify_ecdsa(algorithm, key, msg, sig)
            }
            SigningAlgorithm::Ed25519 => verify_ed25519(key, msg, sig),
            SigningAlgorithm::Ps256 | SigningAlgorithm::Ps384 | SigningAlgorithm::Ps512 => {
                Err(CryptoError::UnsupportedAlgorithm(algorithm))
            }
        }
    }
}

/// Map a key-decoding error.
fn invalid_key(err: impl std::fmt::Display) -> CryptoError {
    CryptoError::InvalidKey(err.to_string())
}

/// Map a signature-decoding error.
fn invalid_signature(err: impl std::fmt::Display) -> CryptoError {
    CryptoError::InvalidSignature(err.to_string())
}

/// Hash `message` with the digest an ECDSA algorithm names (ES256 → SHA-256,
/// ES384 → SHA-384, ES512 → SHA-512), independent of the key's curve.
fn ecdsa_prehash(algorithm: SigningAlgorithm, message: &[u8]) -> Result<Vec<u8>, CryptoError> {
    use sha2::Digest as _;
    match algorithm {
        SigningAlgorithm::Es256 => Ok(sha2::Sha256::digest(message).to_vec()),
        SigningAlgorithm::Es384 => Ok(sha2::Sha384::digest(message).to_vec()),
        SigningAlgorithm::Es512 => Ok(sha2::Sha512::digest(message).to_vec()),
        other => Err(CryptoError::UnsupportedAlgorithm(other)),
    }
}

/// Verify an ES256/ES384/ES512 signature.
///
/// C2PA 2.4, section 13.2 ("Implementations shall accept keys on any of
/// these curves for all ECDSA algorithm choices"): the curve comes from the
/// key (P-256, P-384 or P-521) and the hash from the algorithm.
fn verify_ecdsa(
    algorithm: SigningAlgorithm,
    spki: &[u8],
    message: &[u8],
    signature: &[u8],
) -> Result<(), CryptoError> {
    use p256::pkcs8::DecodePublicKey as _;
    let prehash = ecdsa_prehash(algorithm, message)?;
    if let Ok(key) = p256::ecdsa::VerifyingKey::from_public_key_der(spki) {
        return verify_p256(&key, &prehash, signature);
    }
    if let Ok(key) = p384::ecdsa::VerifyingKey::from_public_key_der(spki) {
        return verify_p384(&key, &prehash, signature);
    }
    match p521::ecdsa::VerifyingKey::from_public_key_der(spki) {
        Ok(key) => verify_p521(&key, &prehash, signature),
        Err(_) => Err(CryptoError::InvalidKey(
            "not a P-256, P-384 or P-521 SubjectPublicKeyInfo".to_string(),
        )),
    }
}

/// Verify a raw `r || s` signature over `prehash` with a P-256 key.
fn verify_p256(
    key: &p256::ecdsa::VerifyingKey,
    prehash: &[u8],
    signature: &[u8],
) -> Result<(), CryptoError> {
    use p256::ecdsa::signature::hazmat::PrehashVerifier as _;
    let sig = p256::ecdsa::Signature::from_slice(signature).map_err(invalid_signature)?;
    key.verify_prehash(prehash, &sig)
        .map_err(|_| CryptoError::VerificationFailed)
}

/// Verify a raw `r || s` signature over `prehash` with a P-384 key.
fn verify_p384(
    key: &p384::ecdsa::VerifyingKey,
    prehash: &[u8],
    signature: &[u8],
) -> Result<(), CryptoError> {
    use p384::ecdsa::signature::hazmat::PrehashVerifier as _;
    let sig = p384::ecdsa::Signature::from_slice(signature).map_err(invalid_signature)?;
    key.verify_prehash(prehash, &sig)
        .map_err(|_| CryptoError::VerificationFailed)
}

/// Verify a raw `r || s` signature over `prehash` with a P-521 key.
fn verify_p521(
    key: &p521::ecdsa::VerifyingKey,
    prehash: &[u8],
    signature: &[u8],
) -> Result<(), CryptoError> {
    use p521::ecdsa::signature::hazmat::PrehashVerifier as _;
    let sig = p521::ecdsa::Signature::from_slice(signature).map_err(invalid_signature)?;
    key.verify_prehash(prehash, &sig)
        .map_err(|_| CryptoError::VerificationFailed)
}

/// Verify an Ed25519 signature with strict (RFC 8032 cofactorless,
/// non-malleable) checking.
fn verify_ed25519(spki: &[u8], message: &[u8], signature: &[u8]) -> Result<(), CryptoError> {
    let key = ed25519_dalek::VerifyingKey::from_public_key_der(spki).map_err(invalid_key)?;
    let sig = ed25519_dalek::Signature::from_slice(signature).map_err(invalid_signature)?;
    key.verify_strict(message, &sig)
        .map_err(|_| CryptoError::VerificationFailed)
}
