//! Digest algorithms (OCFL-S-002, ADR 009; OCFL 1.1 section 3.4): SHA-256
//! is the engine's algorithm, SHA-512 is read, and both match the FIPS 180-4
//! test vectors that WebCrypto and native Rust share (PRD TC-004).

use filesquid_ocfl::digest::DigestAlgorithm;

const SHA256_EMPTY: &str = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";
const SHA256_ABC: &str = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";
const SHA512_EMPTY: &str = "cf83e1357eefb8bdf1542850d66d8007d620e4050b5715dc83f4a921d36ce9ce\
47d0d13c5d85f2b0ff8318d2877eec2f63b931bd47417a81a538327af927da3e";
const SHA512_ABC: &str = "ddaf35a193617abacc417349ae20413112e6fa4e89a97ea20a9eeee64b55d39a\
2192992a274fc1a836ba3c23a3feebbd454d4423643ce80e2a9ac94fa54ca49f";

#[test]
fn sha256_is_the_engine_algorithm() {
    assert_eq!(DigestAlgorithm::ENGINE, DigestAlgorithm::Sha256);
    assert_eq!(DigestAlgorithm::ENGINE.name(), "sha256");
}

#[test]
fn known_vectors_match() {
    assert_eq!(DigestAlgorithm::Sha256.digest_hex(b""), SHA256_EMPTY);
    assert_eq!(DigestAlgorithm::Sha256.digest_hex(b"abc"), SHA256_ABC);
    assert_eq!(DigestAlgorithm::Sha512.digest_hex(b""), SHA512_EMPTY);
    assert_eq!(DigestAlgorithm::Sha512.digest_hex(b"abc"), SHA512_ABC);
}

#[test]
fn streaming_matches_one_shot() {
    let data: Vec<u8> = (0..100_000u32).map(|i| (i % 251) as u8).collect();
    for algorithm in [DigestAlgorithm::Sha256, DigestAlgorithm::Sha512] {
        let mut hasher = algorithm.hasher();
        data.chunks(7_919).for_each(|chunk| hasher.update(chunk));
        assert_eq!(hasher.finalize_hex(), algorithm.digest_hex(&data));
    }
}

#[test]
fn names_and_lengths_follow_ocfl() {
    assert_eq!(
        DigestAlgorithm::from_name("sha256"),
        Some(DigestAlgorithm::Sha256)
    );
    assert_eq!(
        DigestAlgorithm::from_name("sha512"),
        Some(DigestAlgorithm::Sha512)
    );
    for other in ["SHA256", "sha1", "md5", "blake2b-512", ""] {
        assert_eq!(DigestAlgorithm::from_name(other), None, "{other}");
    }
    assert_eq!(DigestAlgorithm::Sha256.hex_len(), 64);
    assert_eq!(DigestAlgorithm::Sha512.hex_len(), 128);
}

#[test]
fn digests_compare_without_regard_to_case() {
    let upper = SHA256_ABC.to_uppercase();
    assert!(DigestAlgorithm::Sha256.matches(&upper, SHA256_ABC));
    assert!(!DigestAlgorithm::Sha256.matches(SHA256_EMPTY, SHA256_ABC));
}

#[test]
fn only_real_digests_match() {
    let not_hex = "z".repeat(64);
    assert!(!DigestAlgorithm::Sha256.matches(&not_hex, &not_hex));
    assert!(!DigestAlgorithm::Sha256.matches("abc", "abc"));
    assert!(!DigestAlgorithm::Sha512.matches(SHA256_ABC, SHA256_ABC));
    assert!(DigestAlgorithm::Sha256.is_digest(SHA256_ABC));
    assert!(!DigestAlgorithm::Sha256.is_digest(&not_hex));
}
