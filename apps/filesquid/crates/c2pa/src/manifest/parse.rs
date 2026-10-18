//! Turning a parsed JUMBF tree into manifest entries (C2PA 2.4 sections
//! 11.1.2 to 11.2 and 15.6).

use std::collections::HashSet;

use super::assertion::{parse_assertion, CborContext};
use super::{
    display_label, requestable, Assertion, Claim, ClaimSignature, ClaimVersion, IgnoreReason,
    IgnoredBox, Manifest, ManifestEntry, ManifestError, ManifestKind, ParseLimits, StoreError,
};
use crate::cbor::{CborError, ItemBudget};
use crate::jumbf::{types, BoxType, ByteRange, ContentBox, JumbfChild, Superbox};

/// Label of the manifest store superbox.
const STORE_LABEL: &str = "c2pa";
/// Label of a `claim-map-v2` claim box.
const CLAIM_V2_LABEL: &str = "c2pa.claim.v2";
/// Label of a legacy `claim-map` claim box.
const CLAIM_V1_LABEL: &str = "c2pa.claim";
/// Label of the claim signature box.
const SIGNATURE_LABEL: &str = "c2pa.signature";
/// Label of the assertion store.
const ASSERTIONS_LABEL: &str = "c2pa.assertions";
/// Path segment shown for a superbox without a label.
const NO_LABEL: &str = "<unlabelled>";

/// Reason text for [`ManifestError::ClaimBox`].
const CLAIM_BOX: &str = "a claim box is requestable and holds exactly one cbor box";
/// Reason text for [`ManifestError::SignatureBox`].
const SIGNATURE_BOX: &str = "a claim signature box is requestable and holds exactly one cbor box";

/// Parse the manifests inside the store superbox `root`, returning them in
/// order with the boxes that were skipped.
pub(super) fn store_contents(
    bytes: &[u8],
    root: &Superbox,
    limits: &ParseLimits,
) -> Result<(Vec<ManifestEntry>, Vec<IgnoredBox>), StoreError> {
    let is_store = root.type_uuid() == types::MANIFEST_STORE && root.label() == Some(STORE_LABEL);
    if !is_store || !requestable(root) {
        return Err(StoreError::NotAManifestStore);
    }
    let mut ctx = Context {
        bytes,
        cbor: CborContext {
            max_depth: limits.max_cbor_depth,
            budget: ItemBudget::new(limits.max_cbor_items),
        },
        ignored: Vec::new(),
    };
    let mut manifests = Vec::new();
    for child in &root.children {
        match child {
            JumbfChild::Content(content) => ctx.ignore_content(STORE_LABEL, content),
            JumbfChild::Superbox(sb) => manifests.extend(ctx.store_child(STORE_LABEL, sb)?),
        }
    }
    if manifests.is_empty() {
        return Err(StoreError::NoManifests);
    }
    let mut seen = HashSet::new();
    if let Some(dup) = manifests.iter().find(|m| !seen.insert(m.label.as_str())) {
        return Err(StoreError::DuplicateLabel {
            label: display_label(&dup.label),
        });
    }
    Ok((manifests, ctx.ignored))
}

/// Why building one manifest stopped.
enum Failure {
    /// The whole store is unusable.
    Store(StoreError),
    /// Only this manifest is unusable.
    Manifest(ManifestError),
}

impl From<StoreError> for Failure {
    fn from(err: StoreError) -> Self {
        Self::Store(err)
    }
}

impl From<ManifestError> for Failure {
    fn from(err: ManifestError) -> Self {
        Self::Manifest(err)
    }
}

impl From<CborError> for Failure {
    /// Only engine-limit CBOR errors reach here; they fail the store.
    fn from(err: CborError) -> Self {
        Self::Store(StoreError::CborLimit(err))
    }
}

/// The claim, signature and assertion store boxes of one manifest.
#[derive(Default)]
struct Parts<'s> {
    /// Claim boxes with their versions.
    claims: Vec<(&'s Superbox, ClaimVersion)>,
    /// Claim signature boxes.
    signatures: Vec<&'s Superbox>,
    /// Assertion stores.
    stores: Vec<&'s Superbox>,
}

/// State while the store's manifests are parsed.
struct Context<'a> {
    /// The store bytes.
    bytes: &'a [u8],
    /// CBOR limits shared by every claim and assertion.
    cbor: CborContext,
    /// Skipped boxes so far.
    ignored: Vec<IgnoredBox>,
}

impl Context<'_> {
    /// Record a content box inside the superbox at `path` as skipped.
    fn ignore_content(&mut self, path: &str, content: &ContentBox) {
        self.ignored.push(IgnoredBox {
            path: path.to_string(),
            box_type: content.box_type,
            type_uuid: None,
            range: content.range,
            reason: IgnoreReason::ContentBox,
        });
    }

    /// Record the superbox `sb`, a child of `parent`, as skipped.
    fn ignore_superbox(&mut self, parent: &str, sb: &Superbox, reason: IgnoreReason) {
        self.ignored.push(IgnoredBox {
            path: child_path(parent, sb),
            box_type: BoxType::JUMB,
            type_uuid: Some(sb.type_uuid()),
            range: sb.range,
            reason,
        });
    }

    /// Turn a superbox directly inside the store into a manifest entry, or
    /// record it as skipped.
    fn store_child(
        &mut self,
        path: &str,
        sb: &Superbox,
    ) -> Result<Option<ManifestEntry>, StoreError> {
        let kind = match sb.type_uuid() {
            types::STANDARD_MANIFEST | types::LEGACY_STANDARD_MANIFEST => ManifestKind::Standard,
            types::UPDATE_MANIFEST => ManifestKind::Update,
            types::COMPRESSED_MANIFEST => ManifestKind::Compressed,
            types::TIMESTAMP_MANIFEST => {
                self.ignore_superbox(path, sb, IgnoreReason::Historical);
                return Ok(None);
            }
            _ => {
                self.ignore_superbox(path, sb, IgnoreReason::UnknownType);
                return Ok(None);
            }
        };
        let label = required_label(sb)?;
        let content = match kind {
            _ if !requestable(sb) => Err(ManifestError::NotRequestable),
            ManifestKind::Compressed => Err(ManifestError::CompressedUnsupported),
            _ => match self.manifest(&child_path(path, sb), sb) {
                Ok(manifest) => Ok(manifest),
                Err(Failure::Manifest(err)) => Err(err),
                Err(Failure::Store(err)) => return Err(err),
            },
        };
        Ok(Some(ManifestEntry {
            label: label.to_string(),
            kind,
            type_uuid: sb.type_uuid(),
            range: sb.range,
            content,
        }))
    }

    /// Build the manifest in `sb`, locating the claim first, then the claim
    /// signature, then the assertion store (C2PA 2.4 sections 15.6 to 15.10).
    fn manifest(&mut self, path: &str, sb: &Superbox) -> Result<Manifest, Failure> {
        let parts = self.parts(path, sb)?;
        let (claim_box, version) = match parts.claims.as_slice() {
            [] => return Err(ManifestError::ClaimMissing.into()),
            [one] => *one,
            _ => return Err(ManifestError::ClaimMultiple.into()),
        };
        let signature_box = pick(
            &parts.signatures,
            ManifestError::SignatureMissing,
            ManifestError::SignatureMultiple,
        )?;
        let store_box = pick(
            &parts.stores,
            ManifestError::AssertionStoreMissing,
            ManifestError::AssertionStoreMultiple,
        )?;
        if !requestable(store_box) {
            return Err(ManifestError::AssertionStoreNotRequestable.into());
        }
        let (claim, claim_cbor) = self.claim(claim_box, version)?;
        let cose_sign1 = only_cbor_box(signature_box)
            .filter(|_| requestable(signature_box))
            .ok_or(ManifestError::SignatureBox {
                reason: SIGNATURE_BOX,
            })?;
        let assertions = self.assertions(path, store_box)?;
        Ok(Manifest {
            claim,
            claim_range: claim_box.range,
            claim_cbor,
            signature: ClaimSignature {
                range: signature_box.range,
                cose_sign1,
            },
            assertion_store: store_box.range,
            assertions,
        })
    }

    /// Sort the children of the manifest superbox `sb` into claim,
    /// signature and assertion store boxes; record every other box as
    /// skipped.
    fn parts<'s>(&mut self, path: &str, sb: &'s Superbox) -> Result<Parts<'s>, StoreError> {
        let mut parts = Parts::default();
        for child in &sb.children {
            match child {
                JumbfChild::Content(content) => self.ignore_content(path, content),
                JumbfChild::Superbox(child) => self.sort_part(path, child, &mut parts)?,
            }
        }
        Ok(parts)
    }

    /// Place one manifest child in `parts`, or record it as skipped.
    fn sort_part<'s>(
        &mut self,
        path: &str,
        child: &'s Superbox,
        parts: &mut Parts<'s>,
    ) -> Result<(), StoreError> {
        let uuid = child.type_uuid();
        let label = match uuid {
            types::CLAIM | types::CLAIM_SIGNATURE | types::ASSERTION_STORE => {
                required_label(child)?
            }
            types::DATABOX_STORE => {
                self.ignore_superbox(path, child, IgnoreReason::Historical);
                return Ok(());
            }
            _ => {
                self.ignore_superbox(path, child, IgnoreReason::UnknownType);
                return Ok(());
            }
        };
        match (uuid, label) {
            (types::CLAIM, CLAIM_V2_LABEL) => parts.claims.push((child, ClaimVersion::V2)),
            (types::CLAIM, CLAIM_V1_LABEL) => parts.claims.push((child, ClaimVersion::V1)),
            (types::CLAIM_SIGNATURE, SIGNATURE_LABEL) => parts.signatures.push(child),
            (types::ASSERTION_STORE, ASSERTIONS_LABEL) => parts.stores.push(child),
            _ => self.ignore_superbox(path, child, IgnoreReason::UnexpectedLabel),
        }
        Ok(())
    }

    /// Decode and check the claim in `sb`, returning it with its CBOR range.
    fn claim(
        &mut self,
        sb: &Superbox,
        version: ClaimVersion,
    ) -> Result<(Claim, ByteRange), Failure> {
        let range = only_cbor_box(sb)
            .filter(|_| requestable(sb))
            .ok_or(ManifestError::ClaimBox { reason: CLAIM_BOX })?;
        let value = self
            .cbor
            .decode(&self.bytes[range.start..range.end])?
            .map_err(ManifestError::ClaimCbor)?;
        let claim = Claim::from_cbor(version, value).map_err(ManifestError::ClaimMalformed)?;
        Ok((claim, range))
    }

    /// Parse every assertion in the assertion store `store`.
    fn assertions(&mut self, path: &str, store: &Superbox) -> Result<Vec<Assertion>, Failure> {
        let store_path = child_path(path, store);
        let mut boxes = Vec::new();
        for child in &store.children {
            match child {
                JumbfChild::Content(content) => self.ignore_content(&store_path, content),
                JumbfChild::Superbox(sb) => boxes.push((required_label(sb)?, sb)),
            }
        }
        if boxes.is_empty() {
            return Err(ManifestError::AssertionStoreEmpty.into());
        }
        let mut seen = HashSet::new();
        if let Some((label, _)) = boxes.iter().find(|(label, _)| !seen.insert(*label)) {
            let label = display_label(label);
            return Err(ManifestError::DuplicateAssertionLabel { label }.into());
        }
        let mut assertions = Vec::with_capacity(boxes.len());
        for (label, sb) in boxes {
            assertions.push(parse_assertion(self.bytes, sb, label, &mut self.cbor)?);
        }
        Ok(assertions)
    }
}

/// The one box in `found`, or `missing` / `multiple`.
fn pick<'s>(
    found: &[&'s Superbox],
    missing: ManifestError,
    multiple: ManifestError,
) -> Result<&'s Superbox, ManifestError> {
    match found {
        [] => Err(missing),
        [one] => Ok(*one),
        _ => Err(multiple),
    }
}

/// The payload of `sb`'s only child when that child is a `cbor` box.
fn only_cbor_box(sb: &Superbox) -> Option<ByteRange> {
    match sb.children.as_slice() {
        [JumbfChild::Content(content)] if content.box_type == BoxType::CBOR => {
            Some(content.range.payload())
        }
        _ => None,
    }
}

/// The label of a C2PA superbox, which the specification requires.
fn required_label(sb: &Superbox) -> Result<&str, StoreError> {
    sb.label().ok_or(StoreError::MissingLabel {
        offset: sb.range.start,
    })
}

/// The display path of `sb` under `parent`.
fn child_path(parent: &str, sb: &Superbox) -> String {
    let label = sb
        .label()
        .map_or_else(|| NO_LABEL.to_string(), display_label);
    format!("{parent}/{label}")
}
