//! Content-addressing digests (OCFL-S-002,
//! [ADR 009](../../../resources/architecture/009-cryptography-webcrypto-compatible-suite.md);
//! OCFL 1.1 section 3.4).
//!
//! OCFL 1.1 objects address content with `sha512` or `sha256`. The engine
//! writes `sha256` ([`DigestAlgorithm::ENGINE`]): it is the algorithm native
//! Rust, WebAssembly and WebCrypto share, with matching test vectors (PRD
//! TC-004). The engine reads both, because OCFL requires readers to accept
//! either and most existing objects use `sha512`.
//!
//! Digests are lowercase hexadecimal when the engine computes them. Base16
//! digests are case insensitive, so [`DigestAlgorithm::matches`] compares
//! without regard to case; inventories keep the case they were written in.

use sha2::{Digest as _, Sha256, Sha512};

/// Lowercase hexadecimal digits.
const HEX: &[u8; 16] = b"0123456789abcdef";

/// An OCFL content-addressing digest algorithm.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum DigestAlgorithm {
    /// SHA-256 (FIPS 180-4): 64 hexadecimal digits. The engine's algorithm.
    Sha256,
    /// SHA-512 (FIPS 180-4): 128 hexadecimal digits. Read, never written.
    Sha512,
}

impl DigestAlgorithm {
    /// The algorithm the engine writes objects with (OCFL-S-002).
    pub const ENGINE: Self = Self::Sha256;

    /// The OCFL name of the algorithm, as used in `digestAlgorithm` and in
    /// sidecar file names.
    pub fn name(self) -> &'static str {
        match self {
            Self::Sha256 => "sha256",
            Self::Sha512 => "sha512",
        }
    }

    /// The algorithm with the OCFL name `name`. Names are case sensitive, as
    /// OCFL writes them; other algorithms are not content-addressing ones.
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "sha256" => Some(Self::Sha256),
            "sha512" => Some(Self::Sha512),
            _ => None,
        }
    }

    /// The number of hexadecimal digits in one digest.
    pub fn hex_len(self) -> usize {
        match self {
            Self::Sha256 => 64,
            Self::Sha512 => 128,
        }
    }

    /// A fresh incremental hasher, for content read in bounded chunks.
    pub fn hasher(self) -> DigestHasher {
        let inner = match self {
            Self::Sha256 => Inner::Sha256(Sha256::new()),
            Self::Sha512 => Inner::Sha512(Sha512::new()),
        };
        DigestHasher { inner }
    }

    /// The lowercase hexadecimal digest of `bytes`.
    pub fn digest_hex(self, bytes: &[u8]) -> String {
        let mut hasher = self.hasher();
        hasher.update(bytes);
        hasher.finalize_hex()
    }

    /// Whether two digests of this algorithm are equal. Base16 is case
    /// insensitive, so `ABC1` matches `abc1`.
    pub fn matches(self, a: &str, b: &str) -> bool {
        a.len() == self.hex_len() && a.eq_ignore_ascii_case(b)
    }
}

/// An incremental digest computation; see [`DigestAlgorithm::hasher`].
#[derive(Debug, Clone)]
pub struct DigestHasher {
    inner: Inner,
}

/// The hasher state for each algorithm.
#[derive(Debug, Clone)]
enum Inner {
    Sha256(Sha256),
    Sha512(Sha512),
}

impl DigestHasher {
    /// The algorithm this hasher computes.
    pub fn algorithm(&self) -> DigestAlgorithm {
        match self.inner {
            Inner::Sha256(_) => DigestAlgorithm::Sha256,
            Inner::Sha512(_) => DigestAlgorithm::Sha512,
        }
    }

    /// Feed the next chunk of content.
    pub fn update(&mut self, chunk: &[u8]) {
        match &mut self.inner {
            Inner::Sha256(hasher) => hasher.update(chunk),
            Inner::Sha512(hasher) => hasher.update(chunk),
        }
    }

    /// Finish and return the lowercase hexadecimal digest.
    pub fn finalize_hex(self) -> String {
        match self.inner {
            Inner::Sha256(hasher) => to_hex(&hasher.finalize()),
            Inner::Sha512(hasher) => to_hex(&hasher.finalize()),
        }
    }
}

/// Lowercase hexadecimal encoding of `bytes`.
pub(crate) fn to_hex(bytes: &[u8]) -> String {
    let mut hex = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        hex.push(char::from(HEX[usize::from(byte >> 4)]));
        hex.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    hex
}
