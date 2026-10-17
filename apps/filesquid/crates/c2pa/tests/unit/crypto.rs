//! C2PA-S-006: cryptographic primitives come from established crates behind
//! the `CryptoBackend` trait. Known-answer vectors (FIPS 180-4 "abc",
//! RFC 8032 test 1) check the default RustCrypto backend; a caller-supplied
//! backend proves the trait can be implemented outside this crate.

use filesquid_c2pa::crypto::{
    CryptoBackend, CryptoError, HashAlgorithm, IncrementalHasher, RustCryptoBackend,
    SigningAlgorithm,
};
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
fn ecdsa_key_of_the_wrong_curve_is_an_invalid_key() {
    let (spki, signature) = p256_signed(b"claim");
    let result = block_on(RustCryptoBackend.verify_signature(
        SigningAlgorithm::Es384,
        &spki,
        b"claim",
        &signature,
    ));
    assert!(
        matches!(result, Err(CryptoError::InvalidKey(_))),
        "{result:?}"
    );
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
