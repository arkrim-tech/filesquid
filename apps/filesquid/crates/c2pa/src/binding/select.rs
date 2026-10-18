//! Choosing the manifest and the hard-binding assertion to validate
//! (C2PA 2.4 sections 9.1, 11.2.3, 15.12 and 18.6.1).
//!
//! Only assertions the claim references (`created_assertions` and
//! `gathered_assertions` in a v2 claim, `assertions` in a v1 claim) are
//! considered. A box in the assertion store that the claim does not name is
//! not part of the signed claim, so it can neither supply the hard binding
//! nor redirect an update manifest to another parent.

use super::{BindingKind, BindingStatus};
use crate::cbor::CborValue;
use crate::jumbf::BoxRange;
use crate::manifest::{
    Assertion, AssertionContent, JumbfUri, Manifest, ManifestKind, ManifestStore,
};

/// The manifest whose binding applies.
pub(super) struct Selected {
    /// Its index in the store.
    pub(super) index: usize,
    /// Whether an update manifest was passed to reach it.
    pub(super) updates_seen: bool,
}

/// The hard-binding assertion of a manifest.
pub(super) struct Binding<'a> {
    /// Its label.
    pub(super) label: String,
    /// Its kind.
    pub(super) kind: BindingKind,
    /// Its CBOR map.
    pub(super) value: &'a CborValue,
}

/// A failure status.
fn invalid(code: &'static str, reason: &'static str) -> BindingStatus {
    BindingStatus::Invalid { code, reason }
}

/// The hard-binding labels, with their kinds. `c2pa.hash.bmff` (v1) is not
/// listed: validators ignore it (section 18.6.1).
const HARD_BINDINGS: [(&str, Option<BindingKind>); 5] = [
    ("c2pa.hash.data", Some(BindingKind::DataHash)),
    ("c2pa.hash.boxes", Some(BindingKind::BoxHash)),
    ("c2pa.hash.bmff.v2", Some(BindingKind::BmffHash)),
    ("c2pa.hash.bmff.v3", Some(BindingKind::BmffHash)),
    ("c2pa.hash.collection.data", None),
];

/// `label` without a `__N` instance suffix.
pub(super) fn base_label(label: &str) -> &str {
    match label.rsplit_once("__") {
        Some((base, n)) if !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()) => base,
        _ => label,
    }
}

/// The assertions of the manifest at `index` that its claim references,
/// in assertion-store order. A reference that does not resolve to one of
/// the manifest's own assertions selects nothing. The hashes in the
/// references are checked by claim validation, not here.
pub(super) fn referenced<'m>(
    store: &ManifestStore,
    index: usize,
    manifest: &'m Manifest,
) -> Vec<&'m Assertion> {
    let claim = &manifest.claim;
    let targets: Vec<BoxRange> = claim
        .created_assertions
        .iter()
        .chain(&claim.gathered_assertions)
        .filter_map(|reference| {
            let uri = JumbfUri::parse(&reference.url).ok()?;
            store.resolve(&uri, index).ok().map(|target| target.range)
        })
        .collect();
    manifest
        .assertions
        .iter()
        .filter(|assertion| targets.contains(&assertion.range))
        .collect()
}

/// The standard manifest whose hard binding applies: the active manifest,
/// or for an update manifest its `parentOf` chain. A manifest may not name
/// itself, but longer cycles are possible; the walk takes at most the
/// store's manifest count of steps, which stops a cycle.
pub(super) fn standard_manifest(store: &ManifestStore) -> Result<Selected, BindingStatus> {
    let manifests = store.manifests();
    let mut index = manifests.len() - 1;
    let mut updates_seen = false;
    for _ in 0..manifests.len() {
        match manifests[index].kind {
            ManifestKind::Standard => {
                return Ok(Selected {
                    index,
                    updates_seen,
                })
            }
            ManifestKind::Compressed => {
                return Err(BindingStatus::NotChecked {
                    reason: "compressed manifests are not supported yet",
                })
            }
            ManifestKind::Update => {}
        }
        let Ok(update) = &manifests[index].content else {
            return Err(BindingStatus::NotChecked {
                reason: "an update manifest on the way to the hard binding is unusable",
            });
        };
        updates_seen = true;
        index = parent_of(store, index, update)
            .map_err(|reason| invalid("claim.hardBindings.missing", reason))?;
    }
    Err(invalid(
        "claim.hardBindings.missing",
        "the update manifests' parents form a loop",
    ))
}

/// The index of the manifest that the update manifest `update` (at
/// `index`) names in its one claim-referenced `parentOf` ingredient, or
/// why there is none.
fn parent_of(
    store: &ManifestStore,
    index: usize,
    update: &Manifest,
) -> Result<usize, &'static str> {
    let assertions = referenced(store, index, update);
    let mut parents = assertions.iter().filter_map(|assertion| {
        let label = base_label(&assertion.label);
        if !matches!(
            label,
            "c2pa.ingredient" | "c2pa.ingredient.v2" | "c2pa.ingredient.v3"
        ) {
            return None;
        }
        match &assertion.content {
            Ok(AssertionContent::Cbor(value)) => parent_url(value),
            _ => None,
        }
    });
    let url = parents
        .next()
        .ok_or("an update manifest has no parentOf ingredient")?;
    if parents.next().is_some() {
        return Err("an update manifest has more than one parentOf ingredient");
    }
    let missing = "an update manifest's parent is not in the store";
    let uri = JumbfUri::parse(url).map_err(|_| missing)?;
    let target = store.resolve(&uri, index).map_err(|_| missing)?;
    store
        .manifests()
        .iter()
        .position(|entry| entry.range == target.range)
        .filter(|parent| *parent != index)
        .ok_or(missing)
}

/// The manifest URL of a `parentOf` ingredient: `activeManifest` (v3) or
/// `c2pa_manifest` (v1, v2).
fn parent_url(value: &CborValue) -> Option<&str> {
    if value.get("relationship")?.as_text()? != "parentOf" {
        return None;
    }
    let reference = value
        .get("activeManifest")
        .or_else(|| value.get("c2pa_manifest"))?;
    reference.get("url")?.as_text()
}

/// The one hard-binding assertion among the claim-referenced `assertions`.
pub(super) fn hard_binding<'a>(assertions: &[&'a Assertion]) -> Result<Binding<'a>, BindingStatus> {
    let mut found = assertions.iter().filter_map(|assertion| {
        let base = base_label(&assertion.label);
        let (_, kind) = HARD_BINDINGS.iter().find(|(label, _)| *label == base)?;
        Some((*assertion, *kind))
    });
    let Some((assertion, kind)) = found.next() else {
        return Err(invalid(
            "claim.hardBindings.missing",
            "the claim references no hard binding",
        ));
    };
    if found.next().is_some() {
        return Err(invalid(
            "assertion.multipleHardBindings",
            "the claim references more than one hard binding",
        ));
    }
    let Some(kind) = kind else {
        return Err(BindingStatus::NotChecked {
            reason: "collection data hashes are not supported yet",
        });
    };
    let value = match &assertion.content {
        Ok(AssertionContent::Cbor(value)) if value.as_map().is_some() => value,
        Ok(_) => {
            return Err(invalid(
                malformed_code(kind),
                "the hard binding is not a CBOR map",
            ))
        }
        Err(err) => {
            let code = err.status_code().unwrap_or(malformed_code(kind));
            return Err(invalid(code, "the hard binding assertion cannot be read"));
        }
    };
    Ok(Binding {
        label: assertion.label.clone(),
        kind,
        value,
    })
}

/// The `*.malformed` code for `kind`.
fn malformed_code(kind: BindingKind) -> &'static str {
    match kind {
        BindingKind::DataHash => "assertion.dataHash.malformed",
        BindingKind::BoxHash => "assertion.boxesHash.malformed",
        BindingKind::BmffHash => "assertion.bmffHash.malformed",
    }
}
