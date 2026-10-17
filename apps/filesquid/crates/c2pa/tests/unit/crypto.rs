//! C2PA-S-006: cryptographic primitives come from established crates behind
//! the `CryptoBackend` trait. Known-answer vectors (FIPS 180-4 "abc",
//! RFC 8032 test 1) check the default RustCrypto backend; a caller-supplied
//! backend proves the trait can be implemented outside this crate.

use std::ops::Range;

use filesquid_c2pa::crypto::{
    CryptoBackend, CryptoError, HashAlgorithm, IncrementalHasher, RustCryptoBackend,
    SigningAlgorithm, DIGEST_CHUNK_BYTES,
};
use filesquid_c2pa::io::{read_exact_at, ByteIoError, ByteSource, MemorySource};
use futures::executor::block_on;

/// Decode a lowercase hex string from a published test vector.
fn hex(text: &str) -> Vec<u8> {
    assert!(text.len().is_multiple_of(2), "even-length hex");
    (0..text.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&text[i..i + 2], 16).expect("hex digit"))
        .collect()
}

/// DER prefix of an Ed25519 SubjectPublicKeyInfo (RFC 8410), before the 32-byte key.
const ED25519_SPKI_PREFIX: &str = "302a300506032b6570032100";

/// RFC 8032 section 7.1, TEST 1 public key.
const RFC8032_T1_PUBLIC: &str = "d75a980182b10ab7d54bfed3c964073a0ee172f3daa62325af021a68f707511a";

/// RFC 8032 section 7.1, TEST 1 signature over the empty message.
const RFC8032_T1_SIGNATURE: &str = "e5564300c360ac729086e2cc806e828a84877f1eb8e5d974d873e065224901555fb8821590a33bacc61e39701cf9b46bd25bf5f0595bbe24655141438e7a100b";

fn ed25519_spki() -> Vec<u8> {
    hex(&format!("{ED25519_SPKI_PREFIX}{RFC8032_T1_PUBLIC}"))
}

#[test]
fn sha256_matches_fips_vector() {
    let digest = block_on(RustCryptoBackend.digest(HashAlgorithm::Sha256, b"abc")).expect("sha256");
    assert_eq!(
        digest,
        hex("ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad")
    );
}

#[test]
fn sha384_matches_fips_vector() {
    let digest = block_on(RustCryptoBackend.digest(HashAlgorithm::Sha384, b"abc")).expect("sha384");
    assert_eq!(
        digest,
        hex("cb00753f45a35e8bb5a03d699ac65007272c32ab0eded1631a8b605a43ff5bed8086072ba1e7cc2358baeca134c825a7")
    );
}

#[test]
fn sha512_matches_fips_vector() {
    let digest = block_on(RustCryptoBackend.digest(HashAlgorithm::Sha512, b"abc")).expect("sha512");
    assert_eq!(
        digest,
        hex("ddaf35a193617abacc417349ae20413112e6fa4e89a97ea20a9eeee64b55d39a2192992a274fc1a836ba3c23a3feebbd454d4423643ce80e2a9ac94fa54ca49f")
    );
}

#[test]
fn incremental_hash_equals_one_shot_hash() {
    let mut hasher = RustCryptoBackend
        .incremental_hasher(HashAlgorithm::Sha256)
        .expect("hasher");
    assert_eq!(hasher.algorithm(), HashAlgorithm::Sha256);
    hasher.update(b"a");
    hasher.update(b"bc");
    let one_shot =
        block_on(RustCryptoBackend.digest(HashAlgorithm::Sha256, b"abc")).expect("digest");
    assert_eq!(hasher.finalize(), one_shot);
}

#[test]
fn hash_algorithm_names_follow_c2pa() {
    for (name, alg, len) in [
        ("sha256", HashAlgorithm::Sha256, 32),
        ("sha384", HashAlgorithm::Sha384, 48),
        ("sha512", HashAlgorithm::Sha512, 64),
    ] {
        assert_eq!(HashAlgorithm::from_c2pa_name(name), Some(alg));
        assert_eq!(alg.c2pa_name(), name);
        assert_eq!(alg.output_len(), len);
    }
    assert_eq!(HashAlgorithm::from_c2pa_name("md5"), None);
}

#[test]
fn signing_algorithms_map_to_cose_identifiers() {
    for (id, name, alg) in [
        (-7, "es256", SigningAlgorithm::Es256),
        (-35, "es384", SigningAlgorithm::Es384),
        (-36, "es512", SigningAlgorithm::Es512),
        (-37, "ps256", SigningAlgorithm::Ps256),
        (-38, "ps384", SigningAlgorithm::Ps384),
        (-39, "ps512", SigningAlgorithm::Ps512),
        (-8, "ed25519", SigningAlgorithm::Ed25519),
    ] {
        assert_eq!(SigningAlgorithm::from_cose_id(id), Some(alg));
        assert_eq!(alg.cose_id(), id);
        assert_eq!(alg.c2pa_name(), name);
    }
    assert_eq!(SigningAlgorithm::from_cose_id(-257), None);
}

#[test]
fn ed25519_accepts_rfc8032_vector() {
    let result = block_on(RustCryptoBackend.verify_signature(
        SigningAlgorithm::Ed25519,
        &ed25519_spki(),
        b"",
        &hex(RFC8032_T1_SIGNATURE),
    ));
    assert_eq!(result, Ok(()));
}

#[test]
fn ed25519_rejects_a_tampered_message() {
    let result = block_on(RustCryptoBackend.verify_signature(
        SigningAlgorithm::Ed25519,
        &ed25519_spki(),
        b"x",
        &hex(RFC8032_T1_SIGNATURE),
    ));
    assert_eq!(result, Err(CryptoError::VerificationFailed));
}

#[test]
fn malformed_public_key_is_reported() {
    let result = block_on(RustCryptoBackend.verify_signature(
        SigningAlgorithm::Ed25519,
        b"not a key",
        b"",
        &hex(RFC8032_T1_SIGNATURE),
    ));
    assert!(
        matches!(result, Err(CryptoError::InvalidKey(_))),
        "{result:?}"
    );
}

#[test]
fn malformed_signature_is_reported() {
    let result = block_on(RustCryptoBackend.verify_signature(
        SigningAlgorithm::Ed25519,
        &ed25519_spki(),
        b"",
        &[1, 2, 3],
    ));
    assert!(
        matches!(result, Err(CryptoError::InvalidSignature(_))),
        "{result:?}"
    );
}

/// Sign `message` with a fixed P-256 key and return (SPKI DER, raw r||s signature).
fn p256_signed(message: &[u8]) -> (Vec<u8>, Vec<u8>) {
    use p256::ecdsa::signature::Signer;
    use p256::pkcs8::EncodePublicKey;
    let key = p256::ecdsa::SigningKey::from_slice(&[7u8; 32]).expect("p256 key");
    let signature: p256::ecdsa::Signature = key.sign(message);
    let spki = key.verifying_key().to_public_key_der().expect("spki");
    (spki.as_bytes().to_vec(), signature.to_bytes().to_vec())
}

/// Sign `message` with a fixed P-384 key and return (SPKI DER, raw r||s signature).
fn p384_signed(message: &[u8]) -> (Vec<u8>, Vec<u8>) {
    use p384::ecdsa::signature::Signer;
    use p384::pkcs8::EncodePublicKey;
    let key = p384::ecdsa::SigningKey::from_slice(&[9u8; 48]).expect("p384 key");
    let signature: p384::ecdsa::Signature = key.sign(message);
    let spki = key.verifying_key().to_public_key_der().expect("spki");
    (spki.as_bytes().to_vec(), signature.to_bytes().to_vec())
}

/// Sign `message` with a fixed P-521 key and return (SPKI DER, raw r||s signature).
fn p521_signed(message: &[u8]) -> (Vec<u8>, Vec<u8>) {
    use p521::ecdsa::signature::Signer;
    use p521::pkcs8::EncodePublicKey;
    let mut secret = [0u8; 66];
    secret[65] = 11;
    let key = p521::ecdsa::SigningKey::from_slice(&secret).expect("p521 key");
    let signature: p521::ecdsa::Signature = key.sign(message);
    let spki = key.verifying_key().to_public_key_der().expect("spki");
    (spki.as_bytes().to_vec(), signature.to_bytes().to_vec())
}

#[test]
fn ecdsa_signatures_verify_for_every_curve() {
    let cases = [
        (SigningAlgorithm::Es256, p256_signed(b"claim")),
        (SigningAlgorithm::Es384, p384_signed(b"claim")),
        (SigningAlgorithm::Es512, p521_signed(b"claim")),
    ];
    for (alg, (spki, signature)) in cases {
        let ok = block_on(RustCryptoBackend.verify_signature(alg, &spki, b"claim", &signature));
        assert_eq!(ok, Ok(()), "{alg:?} valid signature");
        let bad = block_on(RustCryptoBackend.verify_signature(alg, &spki, b"clam", &signature));
        assert_eq!(
            bad,
            Err(CryptoError::VerificationFailed),
            "{alg:?} tampered"
        );
    }
}

#[test]
fn ecdsa_key_that_is_not_a_nist_curve_is_an_invalid_key() {
    let result = block_on(RustCryptoBackend.verify_signature(
        SigningAlgorithm::Es256,
        &ed25519_spki(),
        b"claim",
        &[0u8; 64],
    ));
    assert!(
        matches!(result, Err(CryptoError::InvalidKey(_))),
        "{result:?}"
    );
}

/// Hash `message` with the digest an ECDSA algorithm names.
fn alg_prehash(algorithm: SigningAlgorithm, message: &[u8]) -> Vec<u8> {
    let hash = match algorithm {
        SigningAlgorithm::Es256 => HashAlgorithm::Sha256,
        SigningAlgorithm::Es384 => HashAlgorithm::Sha384,
        _ => HashAlgorithm::Sha512,
    };
    block_on(RustCryptoBackend.digest(hash, message)).expect("prehash")
}

/// Sign the `algorithm` prehash of `message` with a fixed key on `curve`
/// (256, 384 or 521) and return (SPKI DER, raw r||s signature).
fn signed_on_curve(curve: u16, algorithm: SigningAlgorithm, message: &[u8]) -> (Vec<u8>, Vec<u8>) {
    let prehash = alg_prehash(algorithm, message);
    match curve {
        256 => {
            use p256::ecdsa::signature::hazmat::PrehashSigner;
            use p256::pkcs8::EncodePublicKey;
            let key = p256::ecdsa::SigningKey::from_slice(&[7u8; 32]).expect("key");
            let sig: p256::ecdsa::Signature = key.sign_prehash(&prehash).expect("sign");
            let spki = key.verifying_key().to_public_key_der().expect("spki");
            (spki.as_bytes().to_vec(), sig.to_bytes().to_vec())
        }
        384 => {
            use p384::ecdsa::signature::hazmat::PrehashSigner;
            use p384::pkcs8::EncodePublicKey;
            let key = p384::ecdsa::SigningKey::from_slice(&[9u8; 48]).expect("key");
            let sig: p384::ecdsa::Signature = key.sign_prehash(&prehash).expect("sign");
            let spki = key.verifying_key().to_public_key_der().expect("spki");
            (spki.as_bytes().to_vec(), sig.to_bytes().to_vec())
        }
        _ => {
            use p521::ecdsa::signature::hazmat::PrehashSigner;
            use p521::pkcs8::EncodePublicKey;
            let mut secret = [0u8; 66];
            secret[65] = 11;
            let key = p521::ecdsa::SigningKey::from_slice(&secret).expect("key");
            let sig: p521::ecdsa::Signature = key.sign_prehash(&prehash).expect("sign");
            let spki = key.verifying_key().to_public_key_der().expect("spki");
            (spki.as_bytes().to_vec(), sig.to_bytes().to_vec())
        }
    }
}

#[test]
fn every_ecdsa_algorithm_accepts_every_nist_curve() {
    // C2PA 2.4 section 13.2: "Implementations shall accept keys on any of
    // these curves for all ECDSA algorithm choices."
    for algorithm in [
        SigningAlgorithm::Es256,
        SigningAlgorithm::Es384,
        SigningAlgorithm::Es512,
    ] {
        for curve in [256, 384, 521] {
            let (spki, sig) = signed_on_curve(curve, algorithm, b"claim");
            let ok = block_on(RustCryptoBackend.verify_signature(algorithm, &spki, b"claim", &sig));
            assert_eq!(ok, Ok(()), "{algorithm:?} on P-{curve}");
            let bad = block_on(RustCryptoBackend.verify_signature(algorithm, &spki, b"clam", &sig));
            assert_eq!(
                bad,
                Err(CryptoError::VerificationFailed),
                "{algorithm:?} on P-{curve} tampered"
            );
        }
    }
}

#[test]
fn ecdsa_hash_comes_from_the_algorithm_not_the_curve() {
    // An ES256 signature (SHA-256) on a P-384 key must not verify as ES384.
    let (spki, sig) = signed_on_curve(384, SigningAlgorithm::Es256, b"claim");
    let result = block_on(RustCryptoBackend.verify_signature(
        SigningAlgorithm::Es384,
        &spki,
        b"claim",
        &sig,
    ));
    assert_eq!(result, Err(CryptoError::VerificationFailed));
}

#[test]
fn rsa_pss_is_reported_unsupported_until_cp_0_4() {
    for alg in [
        SigningAlgorithm::Ps256,
        SigningAlgorithm::Ps384,
        SigningAlgorithm::Ps512,
    ] {
        let result = block_on(RustCryptoBackend.verify_signature(alg, b"", b"", b""));
        assert_eq!(result, Err(CryptoError::UnsupportedAlgorithm(alg)));
    }
}

/// A caller-supplied backend that only knows SHA-256, built on the default
/// backend's hasher, to prove the trait's default `digest` method.
struct CallerBackend;

impl CryptoBackend for CallerBackend {
    fn incremental_hasher(
        &self,
        algorithm: HashAlgorithm,
    ) -> Result<Box<dyn IncrementalHasher>, CryptoError> {
        if algorithm != HashAlgorithm::Sha256 {
            return Err(CryptoError::UnsupportedHash(algorithm));
        }
        RustCryptoBackend.incremental_hasher(algorithm)
    }

    async fn verify_signature(
        &self,
        algorithm: SigningAlgorithm,
        _public_key_spki_der: &[u8],
        _message: &[u8],
        _signature: &[u8],
    ) -> Result<(), CryptoError> {
        Err(CryptoError::UnsupportedAlgorithm(algorithm))
    }
}

#[test]
fn caller_supplied_backend_uses_the_default_digest() {
    let digest = block_on(CallerBackend.digest(HashAlgorithm::Sha256, b"abc")).expect("digest");
    assert_eq!(digest.len(), 32);
    let refused = block_on(CallerBackend.digest(HashAlgorithm::Sha512, b"abc"));
    assert_eq!(
        refused,
        Err(CryptoError::UnsupportedHash(HashAlgorithm::Sha512))
    );
}

/// Deterministic test bytes spanning several digest chunks.
fn asset_bytes() -> Vec<u8> {
    (0..(DIGEST_CHUNK_BYTES * 2 + 1000))
        .map(|i| (i % 251) as u8)
        .collect()
}

#[test]
fn digest_ranges_streams_included_ranges_across_chunks() {
    let bytes = asset_bytes();
    let len = bytes.len() as u64;
    // Exclude bytes 100..200, as a hard binding's exclusion would.
    let ranges = [0..100, 200..len];
    let mut source = MemorySource::new(bytes.clone());
    let streamed =
        block_on(RustCryptoBackend.digest_ranges(HashAlgorithm::Sha256, &mut source, &ranges))
            .expect("digest");
    let mut included = bytes[..100].to_vec();
    included.extend_from_slice(&bytes[200..]);
    let one_shot =
        block_on(RustCryptoBackend.digest(HashAlgorithm::Sha256, &included)).expect("digest");
    assert_eq!(streamed, one_shot);
}

#[test]
fn digest_ranges_rejects_a_reversed_range() {
    let mut source = MemorySource::new(vec![0u8; 10]);
    #[allow(clippy::reversed_empty_ranges)]
    let reversed: Range<u64> = 5..2;
    let result = block_on(RustCryptoBackend.digest_ranges(
        HashAlgorithm::Sha256,
        &mut source,
        std::slice::from_ref(&reversed),
    ));
    assert_eq!(result, Err(CryptoError::InvalidRange { start: 5, end: 2 }));
}

#[test]
fn digest_ranges_reports_a_short_source() {
    let mut source = MemorySource::new(vec![0u8; 10]);
    let past_end: Range<u64> = 0..20;
    let result = block_on(RustCryptoBackend.digest_ranges(
        HashAlgorithm::Sha256,
        &mut source,
        std::slice::from_ref(&past_end),
    ));
    assert!(
        matches!(
            result,
            Err(CryptoError::Source(ByteIoError::UnexpectedEof { .. }))
        ),
        "{result:?}"
    );
}

/// Largest total input the async-only test backend gathers: 1 MiB.
const MAX_GATHER_BYTES: usize = 1024 * 1024;

/// Append `range` of `source` to `out` in [`DIGEST_CHUNK_BYTES`] chunks,
/// failing once `out` would exceed [`MAX_GATHER_BYTES`].
async fn gather_range<S: ByteSource + ?Sized>(
    source: &mut S,
    range: &Range<u64>,
    out: &mut Vec<u8>,
) -> Result<(), CryptoError> {
    let mut offset = range.start;
    while offset < range.end {
        let take = (range.end - offset).min(DIGEST_CHUNK_BYTES as u64) as usize;
        if out.len() + take > MAX_GATHER_BYTES {
            return Err(CryptoError::Backend("input over the gather limit".into()));
        }
        let start = out.len();
        out.resize(start + take, 0);
        read_exact_at(source, offset, &mut out[start..]).await?;
        offset += take as u64;
    }
    Ok(())
}

/// An async-only backend, shaped like WebCrypto: no incremental hasher, a
/// one-shot async digest, and its own `digest_ranges` that gathers the ranges
/// (here through the default backend, standing in for `subtle.digest`).
struct AsyncOnlyBackend;

impl CryptoBackend for AsyncOnlyBackend {
    fn incremental_hasher(
        &self,
        algorithm: HashAlgorithm,
    ) -> Result<Box<dyn IncrementalHasher>, CryptoError> {
        Err(CryptoError::UnsupportedHash(algorithm))
    }

    async fn digest(&self, algorithm: HashAlgorithm, data: &[u8]) -> Result<Vec<u8>, CryptoError> {
        RustCryptoBackend.digest(algorithm, data).await
    }

    async fn digest_ranges<S: ByteSource + ?Sized>(
        &self,
        algorithm: HashAlgorithm,
        source: &mut S,
        ranges: &[Range<u64>],
    ) -> Result<Vec<u8>, CryptoError> {
        // A one-shot digest needs the bytes together, so gather them in
        // chunks with a hard cap instead of one allocation per range.
        let mut gathered = Vec::new();
        for range in ranges {
            if range.start > range.end {
                return Err(CryptoError::InvalidRange {
                    start: range.start,
                    end: range.end,
                });
            }
            gather_range(source, range, &mut gathered).await?;
        }
        self.digest(algorithm, &gathered).await
    }

    async fn verify_signature(
        &self,
        _algorithm: SigningAlgorithm,
        _public_key_spki_der: &[u8],
        _message: &[u8],
        _signature: &[u8],
    ) -> Result<(), CryptoError> {
        Err(CryptoError::Backend(
            "not implemented by this test backend".into(),
        ))
    }
}

#[test]
fn async_only_backend_can_supply_its_own_range_digest() {
    let bytes = asset_bytes();
    let ranges = [0..100, 200..bytes.len() as u64];
    let mut source = MemorySource::new(bytes.clone());
    let async_only =
        block_on(AsyncOnlyBackend.digest_ranges(HashAlgorithm::Sha384, &mut source, &ranges))
            .expect("digest");
    let mut source = MemorySource::new(bytes);
    let default =
        block_on(RustCryptoBackend.digest_ranges(HashAlgorithm::Sha384, &mut source, &ranges))
            .expect("digest");
    assert_eq!(async_only, default);
    let err = block_on(AsyncOnlyBackend.verify_signature(SigningAlgorithm::Es256, b"", b"", b""));
    assert!(matches!(err, Err(CryptoError::Backend(_))));
}

#[test]
fn async_only_backend_checks_ranges_and_its_gather_limit() {
    let mut source = MemorySource::new(vec![0u8; 10]);
    #[allow(clippy::reversed_empty_ranges)]
    let reversed: Range<u64> = 5..2;
    let result = block_on(AsyncOnlyBackend.digest_ranges(
        HashAlgorithm::Sha256,
        &mut source,
        std::slice::from_ref(&reversed),
    ));
    assert_eq!(result, Err(CryptoError::InvalidRange { start: 5, end: 2 }));
    let big = MAX_GATHER_BYTES as u64 + 1;
    let mut source = MemorySource::new(vec![0u8; big as usize]);
    let too_big: Range<u64> = 0..big;
    let result = block_on(AsyncOnlyBackend.digest_ranges(
        HashAlgorithm::Sha256,
        &mut source,
        std::slice::from_ref(&too_big),
    ));
    assert!(matches!(result, Err(CryptoError::Backend(_))), "{result:?}");
}
