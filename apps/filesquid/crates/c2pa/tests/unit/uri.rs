//! C2PA-S-001: `self#jumbf` URIs in claims and their resolution inside a
//! manifest store (C2PA 2.4 section 8.4.2.1).

#[path = "../support/fixtures.rs"]
#[allow(dead_code)]
mod fixtures;

use filesquid_c2pa::manifest::{JumbfUri, ManifestStore, ParseLimits, ResolveError, UriError};
use fixtures::*;

#[test]
fn relative_and_absolute_uris_parse() {
    let rel = JumbfUri::parse("self#jumbf=c2pa.assertions/c2pa.actions.v2").expect("relative");
    assert!(!rel.is_absolute());
    assert_eq!(rel.segments(), ["c2pa.assertions", "c2pa.actions.v2"]);
    let abs = JumbfUri::parse(&format!("self#jumbf=/c2pa/{ACTIVE_LABEL}/c2pa.signature"))
        .expect("absolute");
    assert!(abs.is_absolute());
    assert_eq!(abs.segments(), ["c2pa", ACTIVE_LABEL, "c2pa.signature"]);
}

#[test]
fn malformed_uris_are_rejected() {
    let cases = [
        ("https://example.org/a.c2pa", UriError::NotSelfJumbf),
        ("self#jumbf=", UriError::Empty),
        ("self#jumbf=/", UriError::EmptySegment),
        (
            "self#jumbf=c2pa.assertions/../c2pa.signature",
            UriError::ParentReference,
        ),
        ("self#jumbf=a..b", UriError::ParentReference),
        ("self#jumbf=a//b", UriError::EmptySegment),
        ("self#jumbf=a/", UriError::EmptySegment),
    ];
    for (text, want) in cases {
        assert_eq!(JumbfUri::parse(text), Err(want), "{text}");
    }
}

#[test]
fn uris_with_too_many_segments_are_rejected() {
    let long = format!("self#jumbf={}", vec!["a"; 17].join("/"));
    assert_eq!(JumbfUri::parse(&long), Err(UriError::TooManySegments));
    let ok = format!("self#jumbf={}", vec!["a"; 16].join("/"));
    assert!(JumbfUri::parse(&ok).is_ok());
}

/// The sample store.
fn sample() -> ManifestStore {
    ManifestStore::from_bytes(sample_store(), &ParseLimits::default()).expect("store")
}

#[test]
fn relative_uris_resolve_from_their_manifest() {
    let store = sample();
    let active = store.manifests().len() - 1;
    let uri = JumbfUri::parse("self#jumbf=c2pa.assertions/c2pa.hash.data").expect("uri");
    let found = store.resolve(&uri, active).expect("resolve");
    let manifest = store.active_manifest().content.as_ref().expect("manifest");
    assert_eq!(found.range, manifest.assertions[1].range);
    let sig = JumbfUri::parse("self#jumbf=c2pa.signature").expect("uri");
    assert_eq!(
        store.resolve(&sig, active).expect("sig").range,
        manifest.signature.range
    );
}

#[test]
fn absolute_uris_resolve_from_the_store() {
    let store = sample();
    let uri = JumbfUri::parse(&format!(
        "self#jumbf=/c2pa/{INGREDIENT_LABEL}/c2pa.assertions/c2pa.actions.v2"
    ))
    .expect("uri");
    let found = store.resolve(&uri, 1).expect("resolve");
    let ingredient = store.manifests()[0].content.as_ref().expect("ingredient");
    assert_eq!(found.range, ingredient.assertions[0].range);
    let whole = JumbfUri::parse(&format!("self#jumbf=/c2pa/{INGREDIENT_LABEL}")).expect("uri");
    assert_eq!(
        store.resolve(&whole, 1).expect("manifest").range,
        store.manifests()[0].range
    );
}

#[test]
fn unresolvable_uris_are_reported() {
    let store = sample();
    let missing = JumbfUri::parse("self#jumbf=c2pa.assertions/absent").expect("uri");
    assert_eq!(
        store.resolve(&missing, 1).map(|_| ()),
        Err(ResolveError::NotFound)
    );
    let other_store = JumbfUri::parse("self#jumbf=/other/x").expect("uri");
    assert_eq!(
        store.resolve(&other_store, 1).map(|_| ()),
        Err(ResolveError::NotFound)
    );
    let rel = JumbfUri::parse("self#jumbf=c2pa.signature").expect("uri");
    assert_eq!(
        store.resolve(&rel, 2).map(|_| ()),
        Err(ResolveError::NoSuchManifest)
    );
}

#[test]
fn ambiguous_labels_are_reported() {
    let dup = superbox(uuid(b"zzzz"), "vendor", &[]);
    let mut children = vec![
        assertion_store(&standard_assertions()),
        claim_box("c2pa.claim.v2", &encode(&claim_v2(&STANDARD_LABELS))),
        signature_box(),
    ];
    children.push(dup.clone());
    children.push(dup);
    let bytes = store(&[manifest(b"c2ma", ACTIVE_LABEL, &children)]);
    let store = ManifestStore::from_bytes(bytes, &ParseLimits::default()).expect("store");
    let uri = JumbfUri::parse("self#jumbf=vendor").expect("uri");
    assert_eq!(
        store.resolve(&uri, 0).map(|_| ()),
        Err(ResolveError::Ambiguous)
    );
}
