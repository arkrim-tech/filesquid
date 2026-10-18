//! NAMASTE conformance declarations (OCFL 1.1 sections 3.2 and 4.2) and
//! inventory digest sidecars (section 3.6).

use filesquid_ocfl::digest::DigestAlgorithm;
use filesquid_ocfl::inventory::{
    check_sidecar_name, sidecar_algorithm, sidecar_contents, sidecar_name, verify_sidecar,
    FormatError, MAX_SIDECAR_BYTES,
};
use filesquid_ocfl::namaste::{Declaration, DeclarationKind, SpecVersion};

const INVENTORY: &[u8] = b"{\"example\": true}\n";

#[test]
fn declarations_have_their_namaste_names_and_contents() {
    let object = Declaration::object(SpecVersion::V1_1);
    assert_eq!(object.file_name(), "0=ocfl_object_1.1");
    assert_eq!(object.contents(), "ocfl_object_1.1\n");
    let root = Declaration::storage_root(SpecVersion::V1_1);
    assert_eq!(root.file_name(), "0=ocfl_1.1");
    assert_eq!(root.contents(), "ocfl_1.1\n");
    assert_eq!(
        SpecVersion::V1_1.inventory_type(),
        "https://ocfl.io/1.1/spec/#inventory"
    );
}

#[test]
fn declaration_file_names_are_recognised() {
    let object = Declaration::from_file_name("0=ocfl_object_1.1").expect("object");
    assert_eq!(
        (object.kind, object.spec),
        (DeclarationKind::Object, SpecVersion::V1_1)
    );
    let root = Declaration::from_file_name("0=ocfl_1.0").expect("root");
    assert_eq!(
        (root.kind, root.spec),
        (DeclarationKind::StorageRoot, SpecVersion::V1_0)
    );
    for other in [
        "1=ocfl_object_1.1",
        "0=ocfl_object_2.0",
        "0=ocfl_object_1.1 ",
        "inventory.json",
    ] {
        assert_eq!(Declaration::from_file_name(other), None, "{other}");
    }
}

#[test]
fn declaration_contents_must_be_exact() {
    let object = Declaration::object(SpecVersion::V1_1);
    assert_eq!(object.check_contents(b"ocfl_object_1.1\n"), Ok(()));
    for bad in [
        &b"ocfl_object_1.1"[..],
        b"ocfl_object_1.1\n\n",
        b"ocfl_object_1.0\n",
        b"",
    ] {
        let err = object.check_contents(bad).expect_err("refused");
        assert_eq!(err.code(), Some("E007"), "{bad:?}");
    }
    let root = Declaration::storage_root(SpecVersion::V1_1);
    assert_eq!(root.check_contents(b"ocfl_1.1\n"), Ok(()));
    assert_eq!(
        root.check_contents(b"ocfl_1.1")
            .expect_err("refused")
            .code(),
        Some("E080")
    );
}

#[test]
fn sidecar_names_carry_the_algorithm() {
    assert_eq!(
        sidecar_name(DigestAlgorithm::Sha256),
        "inventory.json.sha256"
    );
    assert_eq!(sidecar_algorithm("inventory.json.sha512"), Some("sha512"));
    assert_eq!(sidecar_algorithm("inventory.json"), None);
    assert_eq!(
        check_sidecar_name(DigestAlgorithm::Sha256, "inventory.json.sha256"),
        Ok(())
    );
    let err = check_sidecar_name(DigestAlgorithm::Sha256, "inventory.json.sha512");
    assert_eq!(err.expect_err("algorithm mismatch").code(), Some("E059"));
}

#[test]
fn a_written_sidecar_verifies() {
    for algorithm in [DigestAlgorithm::Sha256, DigestAlgorithm::Sha512] {
        let contents = sidecar_contents(algorithm, INVENTORY);
        assert_eq!(
            contents,
            format!("{} inventory.json\n", algorithm.digest_hex(INVENTORY))
        );
        assert_eq!(
            verify_sidecar(algorithm, INVENTORY, contents.as_bytes()),
            Ok(())
        );
    }
}

#[test]
fn sidecar_separators_and_case_are_accepted() {
    let digest = DigestAlgorithm::Sha256.digest_hex(INVENTORY);
    for contents in [
        format!("{digest}\tinventory.json\n"),
        format!("{digest}  \t inventory.json"),
        format!("{} inventory.json\n", digest.to_uppercase()),
    ] {
        let result = verify_sidecar(DigestAlgorithm::Sha256, INVENTORY, contents.as_bytes());
        assert_eq!(result, Ok(()), "{contents:?}");
    }
}

#[test]
fn a_sidecar_with_the_wrong_digest_is_refused() {
    let other = DigestAlgorithm::Sha256.digest_hex(b"something else");
    let contents = format!("{other} inventory.json\n");
    let err = verify_sidecar(DigestAlgorithm::Sha256, INVENTORY, contents.as_bytes());
    assert_eq!(err.expect_err("mismatch").code(), Some("E060"));
}

#[test]
fn an_oversized_sidecar_is_refused_before_it_is_read() {
    let digest = DigestAlgorithm::Sha256.digest_hex(INVENTORY);
    let padding = " ".repeat(MAX_SIDECAR_BYTES);
    let contents = format!("{digest}{padding}inventory.json\n");
    let err = verify_sidecar(DigestAlgorithm::Sha256, INVENTORY, contents.as_bytes());
    let err = err.expect_err("too large");
    assert!(matches!(err, FormatError::TooLarge { .. }), "{err:?}");
    assert_eq!(err.code(), None, "an engine bound has no OCFL code");
}

#[test]
fn a_malformed_sidecar_is_refused() {
    let digest = DigestAlgorithm::Sha256.digest_hex(INVENTORY);
    for contents in [
        format!("{digest}\n"),
        format!("{digest} inventory.json.sha256\n"),
        format!("{digest}inventory.json\n"),
        format!("{digest} inventory.json extra\n"),
        format!("{digest} inventory.json\n\n"),
        String::new(),
        format!(" {digest} inventory.json\n"),
    ] {
        let err = verify_sidecar(DigestAlgorithm::Sha256, INVENTORY, contents.as_bytes());
        assert_eq!(
            err.expect_err("malformed").code(),
            Some("E061"),
            "{contents:?}"
        );
    }
    let bytes = [&[0xffu8, 0xfe][..], b" inventory.json\n"].concat();
    let err = verify_sidecar(DigestAlgorithm::Sha256, INVENTORY, &bytes);
    assert_eq!(err.expect_err("not text").code(), Some("E061"));
}
