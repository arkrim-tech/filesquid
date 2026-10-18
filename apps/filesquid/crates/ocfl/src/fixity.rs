//! Fixity algorithms a validator verifies (OCFL 1.1 section 3.5.4 and the
//! fixity table of section 3.4; OCFL-S-009).
//!
//! OCFL clients must support every algorithm of the fixity table (E027):
//! `md5`, `sha1`, `sha256`, `sha512` and `blake2b-512`. Algorithms a client
//! does not support are ignored (E028), so [`FixityAlgorithm::from_name`]
//! returns `None` for them and the validator skips their values.
//!
//! These algorithms are read only. Objects the engine writes use `sha256`
//! for content addressing and sidecars
//! ([`DigestAlgorithm::ENGINE`](crate::digest::DigestAlgorithm::ENGINE);
//! [ADR 009](../../../../../resources/architecture/009-cryptography-webcrypto-compatible-suite.md));
//! md5 and sha1 are here only to check fixity values other tools wrote.

use blake2::Blake2b512;
use md5::Md5;
use sha1::Sha1;
use sha2::{Digest as _, Sha256, Sha512};

use crate::digest::to_hex;

/// An algorithm of the OCFL 1.1 fixity table.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum FixityAlgorithm {
    /// MD5 (RFC 1321): 32 hexadecimal digits.
    Md5,
    /// SHA-1 (FIPS 180-4): 40 hexadecimal digits.
    Sha1,
    /// SHA-256 (FIPS 180-4): 64 hexadecimal digits.
    Sha256,
    /// SHA-512 (FIPS 180-4): 128 hexadecimal digits.
    Sha512,
    /// BLAKE2b with a 512-bit digest (RFC 7693): 128 hexadecimal digits.
    Blake2b512,
}

impl FixityAlgorithm {
    /// Every algorithm of the fixity table.
    pub const ALL: [Self; 5] = [
        Self::Md5,
        Self::Sha1,
        Self::Sha256,
        Self::Sha512,
        Self::Blake2b512,
    ];

    /// The OCFL name of the algorithm, as a `fixity` block key.
    pub fn name(self) -> &'static str {
        match self {
            Self::Md5 => "md5",
            Self::Sha1 => "sha1",
            Self::Sha256 => "sha256",
            Self::Sha512 => "sha512",
            Self::Blake2b512 => "blake2b-512",
        }
    }

    /// The algorithm with the OCFL name `name` (case sensitive), or `None`
    /// for an algorithm outside the fixity table, which a client ignores.
    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|algorithm| algorithm.name() == name)
    }

    /// A fresh incremental hasher, for content read in bounded chunks.
    pub fn hasher(self) -> FixityHasher {
        let inner = match self {
            Self::Md5 => Inner::Md5(Md5::new()),
            Self::Sha1 => Inner::Sha1(Sha1::new()),
            Self::Sha256 => Inner::Sha256(Sha256::new()),
            Self::Sha512 => Inner::Sha512(Sha512::new()),
            Self::Blake2b512 => Inner::Blake2b512(Blake2b512::new()),
        };
        FixityHasher { inner }
    }

    /// The lowercase hexadecimal digest of `bytes`.
    pub fn digest_hex(self, bytes: &[u8]) -> String {
        let mut hasher = self.hasher();
        hasher.update(bytes);
        hasher.finalize_hex()
    }

    /// The number of hexadecimal digits in one digest.
    pub fn hex_len(self) -> usize {
        match self {
            Self::Md5 => 32,
            Self::Sha1 => 40,
            Self::Sha256 => 64,
            Self::Sha512 | Self::Blake2b512 => 128,
        }
    }

    /// Whether `digest` is a digest of this algorithm: exactly
    /// [`hex_len`](Self::hex_len) ASCII hexadecimal digits, in either case.
    pub fn is_digest(self, digest: &str) -> bool {
        digest.len() == self.hex_len() && digest.bytes().all(|b| b.is_ascii_hexdigit())
    }

    /// Whether `a` and `b` are both digests of this algorithm (see
    /// [`is_digest`](Self::is_digest)) and equal without regard to case.
    /// Fixity digests, unlike state digests, have no exact-case rule.
    pub fn matches(self, a: &str, b: &str) -> bool {
        self.is_digest(a) && self.is_digest(b) && a.eq_ignore_ascii_case(b)
    }
}

/// An incremental fixity computation; see [`FixityAlgorithm::hasher`].
#[derive(Debug, Clone)]
pub struct FixityHasher {
    inner: Inner,
}

/// The hasher state for each algorithm.
#[derive(Debug, Clone)]
enum Inner {
    Md5(Md5),
    Sha1(Sha1),
    Sha256(Sha256),
    Sha512(Sha512),
    Blake2b512(Blake2b512),
}

impl FixityHasher {
    /// Feed the next chunk of content.
    pub fn update(&mut self, chunk: &[u8]) {
        match &mut self.inner {
            Inner::Md5(hasher) => hasher.update(chunk),
            Inner::Sha1(hasher) => hasher.update(chunk),
            Inner::Sha256(hasher) => hasher.update(chunk),
            Inner::Sha512(hasher) => hasher.update(chunk),
            Inner::Blake2b512(hasher) => hasher.update(chunk),
        }
    }

    /// Finish and return the lowercase hexadecimal digest.
    pub fn finalize_hex(self) -> String {
        match self.inner {
            Inner::Md5(hasher) => to_hex(&hasher.finalize()),
            Inner::Sha1(hasher) => to_hex(&hasher.finalize()),
            Inner::Sha256(hasher) => to_hex(&hasher.finalize()),
            Inner::Sha512(hasher) => to_hex(&hasher.finalize()),
            Inner::Blake2b512(hasher) => to_hex(&hasher.finalize()),
        }
    }
}
