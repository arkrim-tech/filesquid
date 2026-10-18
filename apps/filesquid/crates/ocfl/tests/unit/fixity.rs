//! Fixity algorithms a validator must verify (OCFL 1.1 section 3.5.4 and
//! the fixity table, E027; unknown algorithms are ignored, E028): md5,
//! sha1, sha256, sha512 and blake2b-512, checked against known-answer
//! vectors for `abc` (RFC 1321, FIPS 180-4, RFC 7693 Appendix A).

use filesquid_ocfl::fixity::FixityAlgorithm;

/// `(name, digest of "abc")` for every algorithm of the fixity table.
const ABC: [(&str, &str); 5] = [
    ("md5", "900150983cd24fb0d6963f7d28e17f72"),
    ("sha1", "a9993e364706816aba3e25717850c26c9cd0d89d"),
    (
        "sha256",
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad",
    ),
    (
        "sha512",
        "ddaf35a193617abacc417349ae20413112e6fa4e89a97ea20a9eeee64b55d39a\
         2192992a274fc1a836ba3c23a3feebbd454d4423643ce80e2a9ac94fa54ca49f",
    ),
    (
        "blake2b-512",
        "ba80a53f981c4d0d6a2797b69f12f6e94c212f14685ac4b74b12bb6fdbffa2d1\
         7d87c5392aab792dc252d5de4533cc9518d38aa8dbf1925ab92386edd4009923",
    ),
];

#[test]
fn every_fixity_table_algorithm_has_its_known_answer() {
    for (name, expected) in ABC {
        let algorithm = FixityAlgorithm::from_name(name).expect(name);
        assert_eq!(algorithm.name(), name);
        assert_eq!(algorithm.digest_hex(b"abc"), expected, "{name}");
    }
}

#[test]
fn chunked_hashing_equals_one_shot_hashing() {
    for (name, expected) in ABC {
        let algorithm = FixityAlgorithm::from_name(name).expect(name);
        let mut hasher = algorithm.hasher();
        hasher.update(b"a");
        hasher.update(b"");
        hasher.update(b"bc");
        assert_eq!(hasher.finalize_hex(), expected, "{name}");
    }
}

#[test]
fn algorithms_outside_the_table_are_not_supported() {
    for name in ["MD5", "sha-256", "blake2b-160", "size", "crc32", ""] {
        assert_eq!(FixityAlgorithm::from_name(name), None, "{name:?}");
    }
}

#[test]
fn digests_match_without_regard_to_case() {
    let md5 = FixityAlgorithm::from_name("md5").expect("md5");
    assert!(md5.matches(
        "900150983CD24FB0D6963F7D28E17F72",
        "900150983cd24fb0d6963f7d28e17f72"
    ));
    assert!(!md5.matches("900150983cd24fb0d6963f7d28e17f72", "900150983cd24fb0"));
}
