//! Hard-binding validation (C2PA-S-002, C2PA 2.4 section 15.12).
//!
//! [`validate_hard_binding`] checks that a manifest store's hard binding
//! matches the asset's bytes. The binding comes from the active manifest
//! when it is a standard manifest, or from the first standard manifest up
//! its chain of `parentOf` ingredients when it is an update manifest. The
//! report carries the specification's status code:
//! - success: `assertion.dataHash.match`, `assertion.boxesHash.match`,
//!   `assertion.bmffHash.match`;
//! - failure: `*.mismatch`, `*.malformed`, `assertion.boxesHash.unknownBox`,
//!   `algorithm.unsupported`, `claim.hardBindings.missing`,
//!   `assertion.multipleHardBindings`;
//! - informational codes such as `*.additionalExclusionsPresent`.
//!
//! What the engine cannot check yet is reported as
//! [`BindingStatus::NotChecked`] with the reason, never as a pass: collection
//! data hashes, BMFF Merkle trees, compressed manifests, and the
//! multi-asset fallback after a mismatch. All hashing goes through the
//! caller's [`CryptoBackend`], and every asset byte through the caller's
//! [`ByteSource`].

mod bmff;
mod boxes;
mod data;
mod select;

use std::ops::Range;

use crate::cbor::CborValue;
use crate::crypto::{CryptoBackend, CryptoError, HashAlgorithm};
use crate::format::AssetLayout;
use crate::io::{ByteIoError, ByteSource};
use crate::manifest::{Manifest, ManifestStore};

/// The kind of hard binding validated.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BindingKind {
    /// `c2pa.hash.data` (section 18.5).
    DataHash,
    /// `c2pa.hash.boxes` (section 18.7).
    BoxHash,
    /// `c2pa.hash.bmff.v2` or `c2pa.hash.bmff.v3` (section 18.6).
    BmffHash,
}

/// The outcome of a hard-binding check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BindingStatus {
    /// The binding matches; `code` is the success code.
    Valid {
        /// For example `assertion.dataHash.match`.
        code: &'static str,
    },
    /// The binding fails; `code` is the failure code.
    Invalid {
        /// For example `assertion.dataHash.mismatch`.
        code: &'static str,
        /// What was found, in fixed text.
        reason: &'static str,
    },
    /// The engine cannot check this binding yet. Not a pass.
    NotChecked {
        /// Why, in fixed text.
        reason: &'static str,
    },
}

impl BindingStatus {
    /// Whether the binding was checked and matches.
    pub fn is_valid(&self) -> bool {
        matches!(self, Self::Valid { .. })
    }
}

/// The result of [`validate_hard_binding`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BindingReport {
    /// The label of the manifest whose binding was checked.
    pub manifest: Option<String>,
    /// The hard-binding assertion's label.
    pub assertion: Option<String>,
    /// The kind of binding.
    pub kind: Option<BindingKind>,
    /// The outcome.
    pub status: BindingStatus,
    /// Informational codes, such as `assertion.dataHash.additionalExclusionsPresent`.
    pub informational: Vec<&'static str>,
}

impl BindingReport {
    /// The success or failure code; `None` when not checked.
    pub fn code(&self) -> Option<&'static str> {
        match &self.status {
            BindingStatus::Valid { code } | BindingStatus::Invalid { code, .. } => Some(code),
            BindingStatus::NotChecked { .. } => None,
        }
    }
}

/// Why validation could not run at all (as opposed to a binding failing).
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum BindingError {
    /// The asset could not be read.
    #[error("reading the asset failed: {0}")]
    Io(#[from] ByteIoError),
    /// The crypto backend failed.
    #[error("hashing failed: {0}")]
    Crypto(CryptoError),
}

impl From<CryptoError> for BindingError {
    fn from(err: CryptoError) -> Self {
        match err {
            CryptoError::Source(io) => Self::Io(io),
            other => Self::Crypto(other),
        }
    }
}

/// The outcome of one binding check, before it is put in a report.
pub(crate) struct Outcome {
    /// The status.
    pub(crate) status: BindingStatus,
    /// Informational codes.
    pub(crate) informational: Vec<&'static str>,
}

impl Outcome {
    /// A failure with no informational codes.
    pub(crate) fn invalid(code: &'static str, reason: &'static str) -> Self {
        Self {
            status: BindingStatus::Invalid { code, reason },
            informational: Vec::new(),
        }
    }

    /// `matched` mapped to `success` or `failure`, with `informational`.
    pub(crate) fn compared(
        matched: bool,
        success: &'static str,
        failure: &'static str,
        informational: Vec<&'static str>,
    ) -> Self {
        let status = match matched {
            true => BindingStatus::Valid { code: success },
            false => BindingStatus::Invalid {
                code: failure,
                reason: "the computed hash differs from the recorded hash",
            },
        };
        Self {
            status,
            informational,
        }
    }
}

/// What every binding check needs.
pub(crate) struct Check<'a, S: ?Sized, C> {
    /// The asset.
    pub(crate) asset: &'a mut S,
    /// The hashing backend.
    pub(crate) crypto: &'a C,
    /// The asset's layout, when a format handler produced one.
    pub(crate) layout: Option<&'a AssetLayout>,
    /// The claim's `alg`, the last fallback for the hash algorithm.
    pub(crate) claim_alg: Option<&'a str>,
    /// Whether an update manifest was passed on the way to this binding.
    pub(crate) updates_seen: bool,
}

/// Validate the hard binding of `store` against the asset in `asset`.
///
/// Pass the `layout` from [`crate::read::read_asset`] when the store is
/// embedded in the asset: data hashes need the store's location and box
/// hashes need the boxes. For a sidecar or remote store pass `None`.
pub async fn validate_hard_binding<S, C>(
    store: &ManifestStore,
    asset: &mut S,
    layout: Option<&AssetLayout>,
    crypto: &C,
) -> Result<BindingReport, BindingError>
where
    S: ByteSource + ?Sized,
    C: CryptoBackend,
{
    let selected = match select::standard_manifest(store) {
        Ok(selected) => selected,
        Err(status) => return Ok(report(None, None, None, status)),
    };
    let entry = &store.manifests()[selected.index];
    let label = Some(entry.label.clone());
    let Ok(manifest) = &entry.content else {
        let status = BindingStatus::NotChecked {
            reason: "the manifest with the hard binding is unusable",
        };
        return Ok(report(label, None, None, status));
    };
    let binding = match select::hard_binding(manifest) {
        Ok(binding) => binding,
        Err(status) => return Ok(report(label, None, None, status)),
    };
    let mut check = Check {
        asset,
        crypto,
        layout,
        claim_alg: manifest.claim.alg.as_deref(),
        updates_seen: selected.updates_seen,
    };
    let outcome = match binding.kind {
        BindingKind::DataHash => data::validate(&mut check, binding.value).await?,
        BindingKind::BoxHash => boxes::validate(&mut check, binding.value).await?,
        BindingKind::BmffHash => bmff::validate(&mut check, binding.value).await?,
    };
    let outcome = multi_asset_fallback(manifest, outcome);
    let mut out = report(
        label,
        Some(binding.label),
        Some(binding.kind),
        outcome.status,
    );
    out.informational = outcome.informational;
    Ok(out)
}

/// A report without informational codes.
fn report(
    manifest: Option<String>,
    assertion: Option<String>,
    kind: Option<BindingKind>,
    status: BindingStatus,
) -> BindingReport {
    BindingReport {
        manifest,
        assertion,
        kind,
        status,
        informational: Vec::new(),
    }
}

/// After a hash mismatch, a multi-asset hash assertion decides instead
/// (sections 15.12.1.1, 15.12.2, 15.12.3); it is not supported yet, so the
/// binding is reported as not checked rather than as a mismatch.
fn multi_asset_fallback(manifest: &Manifest, outcome: Outcome) -> Outcome {
    let mismatch = matches!(
        outcome.status,
        BindingStatus::Invalid { code, .. } if code.ends_with("Hash.mismatch")
    );
    let has_multi = manifest
        .assertions
        .iter()
        .any(|a| select::base_label(&a.label) == "c2pa.hash.multi-asset");
    if !(mismatch && has_multi) {
        return outcome;
    }
    Outcome {
        status: BindingStatus::NotChecked {
            reason: "a multi-asset hash assertion applies and is not supported yet",
        },
        informational: outcome.informational,
    }
}

/// The hash algorithm for a structure: the first present `alg` among
/// `candidates` (nearest first), else the claim's `alg` (sections 13.1 and
/// 15.4). `None` means `algorithm.unsupported`: no `alg` anywhere, a
/// non-text `alg`, or a name outside the allowed list.
pub(crate) fn hash_algorithm(
    candidates: &[Option<&CborValue>],
    claim_alg: Option<&str>,
) -> Option<HashAlgorithm> {
    match candidates.iter().flatten().next() {
        Some(field) => HashAlgorithm::from_c2pa_name(field.as_text()?),
        None => HashAlgorithm::from_c2pa_name(claim_alg?),
    }
}

/// `whole` minus the sorted, non-overlapping `excluded` ranges (clipped to
/// `whole`).
pub(crate) fn included_ranges(whole: Range<u64>, excluded: &[Range<u64>]) -> Vec<Range<u64>> {
    let mut out = Vec::new();
    let mut at = whole.start;
    for range in excluded {
        let start = range.start.clamp(whole.start, whole.end);
        let end = range.end.clamp(whole.start, whole.end);
        if start > at {
            out.push(at..start);
        }
        at = at.max(end);
    }
    if at < whole.end {
        out.push(at..whole.end);
    }
    out
}
