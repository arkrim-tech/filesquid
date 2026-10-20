//! Namespace registrations persist in canonical OCFL state (MD-0.7,
//! MD-S-012): the `ocfl` store commits the registry record at the caller's
//! logical path, one version per change, and refuses a save that drops a
//! registration or changes its IRI, so a rebuilt index recovers them.

use filesquid_metadata::file_ref::FileRef;
use filesquid_metadata::namespace::{
    NamespaceRegistry, Registrant, Registration, RegistryError, NAMESPACES_MD,
};
use filesquid_metadata::store::ocfl::CommitOptions;
use filesquid_metadata::store::StoreError;
use filesquid_ocfl::validate::validate_object;

#[path = "../support/ocfl_rig.rs"]
mod rig;

use rig::{object_root, ready, Rig};

/// The object that holds the repository's registry.
const REGISTRY_OBJECT: &str = "urn:example:repository";

/// The registry record of [`REGISTRY_OBJECT`].
fn registry_ref() -> FileRef {
    FileRef::new(REGISTRY_OBJECT, NAMESPACES_MD)
        .unwrap()
        .with_record_path(NAMESPACES_MD)
        .unwrap()
}

/// A registry holding the `rico` extension.
fn with_rico() -> NamespaceRegistry {
    let mut registry = NamespaceRegistry::default();
    registry
        .register(Registration::extension(
            "rico",
            "https://www.ica.org/standards/RiC/ontology#",
            Some(vec!["title".to_string()]),
            Registrant::Distribution("arkrim".to_string()),
        ))
        .unwrap();
    registry
}

#[test]
fn an_object_without_a_registry_reads_as_empty() {
    let rig = Rig::memory();
    let registry = ready(rig.store().read_registry(&registry_ref())).unwrap();
    assert!(registry.registrations().is_empty());
}

#[test]
fn a_registration_is_committed_in_one_version_and_read_back() {
    let rig = Rig::memory();
    let receipt = ready(rig.store().commit_registry_with(
        &registry_ref(),
        &with_rico(),
        &CommitOptions::default(),
    ))
    .unwrap()
    .expect("a version");
    assert_eq!(receipt.version.to_string(), "v1");
    let back = ready(rig.store().read_registry(&registry_ref())).unwrap();
    assert_eq!(back, with_rico());
    let report = ready(validate_object(&rig.storage, &object_root(REGISTRY_OBJECT))).unwrap();
    assert!(report.is_valid(), "{:?}", report.errors());
}

#[test]
fn an_unchanged_registry_publishes_nothing() {
    let rig = Rig::memory();
    let options = CommitOptions::default();
    ready(
        rig.store()
            .commit_registry_with(&registry_ref(), &with_rico(), &options),
    )
    .unwrap();
    let again = ready(
        rig.store()
            .commit_registry_with(&registry_ref(), &with_rico(), &options),
    );
    assert!(matches!(again, Ok(None)), "{again:?}");
    assert_eq!(rig.versions(REGISTRY_OBJECT), ["v1"]);
}

#[test]
fn a_save_that_drops_a_registration_is_refused_and_publishes_nothing() {
    let rig = Rig::memory();
    let options = CommitOptions::default();
    ready(
        rig.store()
            .commit_registry_with(&registry_ref(), &with_rico(), &options),
    )
    .unwrap();
    let empty = NamespaceRegistry::default();
    let result = ready(
        rig.store()
            .commit_registry_with(&registry_ref(), &empty, &options),
    );
    assert!(
        matches!(
            &result,
            Err(StoreError::Registry(RegistryError::RegistrationRemoved(prefix))) if prefix == "rico"
        ),
        "{result:?}"
    );
    assert_eq!(rig.versions(REGISTRY_OBJECT), ["v1"]);
}

#[test]
fn a_new_registration_is_a_new_version() {
    let rig = Rig::memory();
    let options = CommitOptions::default();
    ready(
        rig.store()
            .commit_registry_with(&registry_ref(), &with_rico(), &options),
    )
    .unwrap();
    let mut grown = ready(rig.store().read_registry(&registry_ref())).unwrap();
    grown
        .register(Registration::custom(
            "mine",
            "urn:uuid:0b5c4d1e-8a6f-4b21-9a0e-1f2d3c4b5a69",
        ))
        .unwrap();
    ready(
        rig.store()
            .commit_registry_with(&registry_ref(), &grown, &options),
    )
    .unwrap();
    assert_eq!(rig.versions(REGISTRY_OBJECT), ["v1", "v2"]);
    let pinned = ready(
        rig.store()
            .read_registry(&registry_ref().with_version("v1")),
    )
    .unwrap();
    assert_eq!(pinned, with_rico());
}
