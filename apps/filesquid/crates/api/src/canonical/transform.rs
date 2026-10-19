//! The dual-profile seam (ADR 010, ADR 023): the engine stores the bytes the caller gives
//! it; a profile transforms them before staging and after reading. The owner-readable local
//! profile is the identity ([`OwnerReadable`]). The encrypted profile seals names and
//! content per ADR 009 and plugs in here once that cipher exists.

use super::CanonicalError;

/// A profile's transform of one logical file's bytes.
pub trait ProfileTransform: Send + Sync {
    /// The bytes to stage for the plain `bytes` of logical `path`.
    ///
    /// # Errors
    /// [`CanonicalError::Encode`] when they cannot be transformed.
    fn seal(&self, path: &str, bytes: Vec<u8>) -> Result<Vec<u8>, CanonicalError>;

    /// The plain bytes of logical `path` from its `stored` bytes.
    ///
    /// # Errors
    /// [`CanonicalError::Decode`] when they cannot be transformed back.
    fn open(&self, path: &str, stored: Vec<u8>) -> Result<Vec<u8>, CanonicalError>;
}

/// The owner-readable local profile: bytes are stored as they are, so the library can be
/// inspected and copied with ordinary tools (F-001, ADR 010).
#[derive(Clone, Copy, Debug, Default)]
pub struct OwnerReadable;

impl ProfileTransform for OwnerReadable {
    fn seal(&self, _path: &str, bytes: Vec<u8>) -> Result<Vec<u8>, CanonicalError> {
        Ok(bytes)
    }

    fn open(&self, _path: &str, stored: Vec<u8>) -> Result<Vec<u8>, CanonicalError> {
        Ok(stored)
    }
}
