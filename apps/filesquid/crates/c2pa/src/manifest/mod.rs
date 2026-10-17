//! C2PA manifest stores, manifests, claims and assertions (C2PA-S-001).
//!
//! [`ManifestStore::from_bytes`] parses a complete C2PA Manifest Store, the
//! JUMBF superbox labelled `c2pa` (C2PA 2.4 section 11.1.4.2), as found in a
//! sidecar `.c2pa` file, a remote store or an asset's embedded data. Errors
//! come in three layers, so one damaged manifest never hides the others:
//!
//! - [`StoreError`]: the store itself cannot be trusted (broken JUMBF
//!   framing, not a manifest store, no manifests, an engine limit). Nothing
//!   is returned.
//! - [`ManifestError`]: one manifest is unusable (for example no claim, or a
//!   claim that is not valid CBOR). Its [`ManifestEntry::content`] holds the
//!   error, with the specification's failure code from
//!   [`ManifestError::status_code`]; the other manifests are still parsed.
//! - [`AssertionError`]: one assertion's content is unusable. Its
//!   [`Assertion::content`] holds the error; the manifest is still parsed,
//!   and validation decides whether the claim references it.
//!
//! Boxes the specification says to skip (unknown type UUIDs, historical
//! time-stamp manifests and data box stores) are listed in
//! [`ManifestStore::ignored`] rather than dropped silently. Nothing here
//! verifies hashes or signatures; the byte ranges kept for that are used by
//! the validation stages.

mod assertion;
mod claim;
mod parse;
mod uri;

pub use assertion::{Assertion, AssertionContent, AssertionError};
pub use claim::{Claim, ClaimError, ClaimVersion, GeneratorInfo, HashedUri};
pub use uri::{JumbfUri, ResolveError, UriError, MAX_URI_SEGMENTS};

use crate::cbor::CborError;
use crate::jumbf::{BoxRange, BoxType, ByteRange, JumbfError, JumbfLimits, Superbox, TypeUuid};

/// Default [`ParseLimits::max_store_bytes`]: 64 MiB.
pub const DEFAULT_MAX_STORE_BYTES: u64 = 64 * 1024 * 1024;
/// Default [`ParseLimits::max_box_depth`].
pub const DEFAULT_MAX_BOX_DEPTH: usize = 16;
/// Default [`ParseLimits::max_boxes`].
pub const DEFAULT_MAX_BOXES: usize = 65_536;
/// Default [`ParseLimits::max_cbor_depth`].
pub const DEFAULT_MAX_CBOR_DEPTH: usize = 64;
/// Default [`ParseLimits::max_cbor_items`].
pub const DEFAULT_MAX_CBOR_ITEMS: usize = 1_048_576;

/// Bounds on the work and memory one manifest store may cost. Exceeding any
/// of them fails the whole store with a typed error; nothing is clamped or
/// skipped.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseLimits {
    /// Largest store, in bytes, that is read or fetched. Checked before the
    /// store is read or allocated; remote fetches use it as `max_bytes`.
    pub max_store_bytes: u64,
    /// Deepest JUMBF superbox nesting; the store superbox is depth 1.
    /// Values above [`crate::jumbf::MAX_SUPPORTED_DEPTH`] act as that value.
    pub max_box_depth: usize,
    /// Most JUMBF boxes in one store, counting every box header.
    pub max_boxes: usize,
    /// Deepest CBOR nesting in one claim or assertion; a scalar is depth 1.
    /// Values above [`crate::cbor::MAX_SUPPORTED_DEPTH`] act as that value.
    pub max_cbor_depth: usize,
    /// Most CBOR data items decoded across the whole store.
    pub max_cbor_items: usize,
}

impl Default for ParseLimits {
    fn default() -> Self {
        Self {
            max_store_bytes: DEFAULT_MAX_STORE_BYTES,
            max_box_depth: DEFAULT_MAX_BOX_DEPTH,
            max_boxes: DEFAULT_MAX_BOXES,
            max_cbor_depth: DEFAULT_MAX_CBOR_DEPTH,
            max_cbor_items: DEFAULT_MAX_CBOR_ITEMS,
        }
    }
}

impl ParseLimits {
    /// The JUMBF part of these limits.
    fn jumbf(&self) -> JumbfLimits {
        JumbfLimits {
            max_depth: self.max_box_depth,
            max_boxes: self.max_boxes,
        }
    }
}

/// The kind of a manifest, from its JUMBF type UUID (C2PA 2.4 section 11.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ManifestKind {
    /// A standard manifest (`c2ma`, or the older `c2md`).
    Standard,
    /// An update manifest (`c2um`).
    Update,
    /// A Brotli-compressed manifest (`c2cm`). Decompression is not
    /// implemented yet, so its content is
    /// [`ManifestError::CompressedUnsupported`].
    Compressed,
}

/// The claim signature box of a manifest.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClaimSignature {
    /// The `c2pa.signature` superbox.
    pub range: BoxRange,
    /// The content of its CBOR box: the `COSE_Sign1_Tagged` structure,
    /// unverified (signature validation is a separate stage).
    pub cose_sign1: ByteRange,
}

/// A manifest whose claim, claim signature and assertion store were found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Manifest {
    /// The parsed claim.
    pub claim: Claim,
    /// The claim superbox.
    pub claim_range: BoxRange,
    /// The content of the claim's CBOR box: the bytes the claim signature
    /// signs (C2PA 2.4 section 10.3.2.4).
    pub claim_cbor: ByteRange,
    /// The claim signature.
    pub signature: ClaimSignature,
    /// The `c2pa.assertions` superbox.
    pub assertion_store: BoxRange,
    /// Every assertion in the assertion store, in order.
    pub assertions: Vec<Assertion>,
}

/// One manifest in a store, parsed or not.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManifestEntry {
    /// The manifest label (normally a `urn:c2pa:` identifier).
    pub label: String,
    /// The manifest kind.
    pub kind: ManifestKind,
    /// The JUMBF type UUID it was recognised by.
    pub type_uuid: TypeUuid,
    /// The manifest superbox.
    pub range: BoxRange,
    /// The manifest, or why it cannot be used.
    pub content: Result<Manifest, ManifestError>,
}

/// Why a box was skipped (C2PA 2.4 section 11.1.2 says unknown boxes are
/// ignored; this crate reports each one instead of dropping it silently).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IgnoreReason {
    /// The superbox's type UUID is not one C2PA defines.
    UnknownType,
    /// A historical structure that manifest consumers do not read: a
    /// time-stamp manifest (`c2tm`) or a data box store (`c2db`).
    Historical,
    /// A C2PA type UUID with a label the specification does not give it
    /// (for example a `c2cl` box not labelled `c2pa.claim.v2` or
    /// `c2pa.claim`).
    UnexpectedLabel,
    /// A content box where only superboxes are defined.
    ContentBox,
}

/// A box the parser skipped.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IgnoredBox {
    /// Labels from the store down to the box, joined by `/`, each escaped
    /// and shortened for display. A content box has its container's path.
    pub path: String,
    /// The box type.
    pub box_type: BoxType,
    /// The type UUID, for a superbox.
    pub type_uuid: Option<TypeUuid>,
    /// Where the box is.
    pub range: BoxRange,
    /// Why it was skipped.
    pub reason: IgnoreReason,
}

/// Why one manifest cannot be used. [`ManifestError::status_code`] gives
/// the C2PA 2.4 failure code a validator reports for it.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ManifestError {
    /// A compressed manifest; Brotli decompression is not implemented.
    #[error("compressed manifests are not supported yet")]
    CompressedUnsupported,
    /// No claim box (`claim.missing`).
    #[error("the manifest has no claim")]
    ClaimMissing,
    /// More than one claim box (`claim.multiple`).
    #[error("the manifest has more than one claim")]
    ClaimMultiple,
    /// The claim box does not hold exactly one CBOR box (`claim.malformed`).
    #[error("the claim box is malformed: {reason}")]
    ClaimBox {
        /// Which rule was broken.
        reason: &'static str,
    },
    /// The claim is not valid CBOR (`claim.cbor.invalid`).
    #[error("the claim is not valid CBOR: {0}")]
    ClaimCbor(CborError),
    /// A claim field is missing or has the wrong type (`claim.malformed`).
    #[error("the claim is malformed: {0}")]
    ClaimMalformed(ClaimError),
    /// No claim signature box (`claimSignature.missing`).
    #[error("the manifest has no claim signature")]
    SignatureMissing,
    /// More than one claim signature box, so the claim's signature reference
    /// cannot be resolved (`claimSignature.missing`).
    #[error("the manifest has more than one claim signature")]
    SignatureMultiple,
    /// The claim signature box does not hold exactly one CBOR box
    /// (`claimSignature.missing`).
    #[error("the claim signature box is malformed: {reason}")]
    SignatureBox {
        /// Which rule was broken.
        reason: &'static str,
    },
    /// No assertion store (`assertion.missing`).
    #[error("the manifest has no assertion store")]
    AssertionStoreMissing,
    /// An assertion store without assertions (`assertion.missing`).
    #[error("the assertion store is empty")]
    AssertionStoreEmpty,
    /// More than one assertion store, so assertion references cannot be
    /// resolved (`assertion.missing`).
    #[error("the manifest has more than one assertion store")]
    AssertionStoreMultiple,
    /// Two assertions share a label, so references to it are ambiguous.
    #[error("two assertions share the label {label}")]
    DuplicateAssertionLabel {
        /// The label, escaped and shortened for display.
        label: String,
    },
}

impl ManifestError {
    /// The C2PA 2.4 failure code for this error (section 15), or `None`
    /// where the specification defines none.
    pub fn status_code(&self) -> Option<&'static str> {
        match self {
            Self::ClaimMissing => Some("claim.missing"),
            Self::ClaimMultiple => Some("claim.multiple"),
            Self::ClaimBox { .. } | Self::ClaimMalformed(_) => Some("claim.malformed"),
            Self::ClaimCbor(_) => Some("claim.cbor.invalid"),
            Self::SignatureMissing | Self::SignatureMultiple | Self::SignatureBox { .. } => {
                Some("claimSignature.missing")
            }
            Self::AssertionStoreMissing
            | Self::AssertionStoreEmpty
            | Self::AssertionStoreMultiple => Some("assertion.missing"),
            Self::CompressedUnsupported | Self::DuplicateAssertionLabel { .. } => None,
        }
    }
}

/// Why a manifest store cannot be used at all.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum StoreError {
    /// The JUMBF framing is broken or exceeds a box limit.
    #[error("invalid JUMBF: {0}")]
    Jumbf(#[from] JumbfError),
    /// CBOR in the store exceeds the depth or item limit.
    #[error("CBOR limit exceeded: {0}")]
    CborLimit(CborError),
    /// The superbox is not labelled `c2pa` with the manifest store UUID.
    #[error("the superbox is not a C2PA manifest store")]
    NotAManifestStore,
    /// The store holds no manifest.
    #[error("the manifest store holds no manifests")]
    NoManifests,
    /// Two manifests share a label.
    #[error("two manifests share the label {label}")]
    DuplicateLabel {
        /// The label, escaped and shortened for display.
        label: String,
    },
    /// A C2PA box (manifest, claim, signature, assertion store or assertion)
    /// has no label, so it cannot be referenced.
    #[error("the C2PA box at offset {offset} has no label")]
    MissingLabel {
        /// Start of the superbox.
        offset: usize,
    },
}

impl StoreError {
    /// A fixed category name for logs and metrics.
    pub fn category(&self) -> &'static str {
        match self {
            Self::Jumbf(err) => err.category(),
            Self::CborLimit(err) => err.category(),
            Self::NotAManifestStore => "not_a_manifest_store",
            Self::NoManifests => "no_manifests",
            Self::DuplicateLabel { .. } => "duplicate_label",
            Self::MissingLabel { .. } => "missing_label",
        }
    }
}

/// A parsed C2PA Manifest Store, holding its bytes so that every
/// [`ByteRange`] and [`BoxRange`] it reports can be sliced.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManifestStore {
    /// The store bytes.
    bytes: Vec<u8>,
    /// The JUMBF tree.
    root: Superbox,
    /// Manifests in store order; never empty.
    manifests: Vec<ManifestEntry>,
    /// Skipped boxes, in the order they were met.
    ignored: Vec<IgnoredBox>,
}

impl ManifestStore {
    /// Parse `bytes` as a complete manifest store within `limits`.
    pub fn from_bytes(bytes: Vec<u8>, limits: &ParseLimits) -> Result<Self, StoreError> {
        let root = crate::jumbf::parse_superbox(&bytes, &limits.jumbf())?;
        let (manifests, ignored) = parse::store_contents(&bytes, &root, limits)?;
        Ok(Self {
            bytes,
            root,
            manifests,
            ignored,
        })
    }

    /// The store bytes.
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// The bytes of `range`, or `None` when the range does not lie inside
    /// this store (for example a range taken from another store).
    pub fn slice(&self, range: ByteRange) -> Option<&[u8]> {
        self.bytes.get(range.start..range.end)
    }

    /// The store's JUMBF tree.
    pub fn root(&self) -> &Superbox {
        &self.root
    }

    /// Every manifest, in store order.
    pub fn manifests(&self) -> &[ManifestEntry] {
        &self.manifests
    }

    /// The active manifest: the last manifest in the store (C2PA 2.4
    /// section 11.1.4.2).
    pub fn active_manifest(&self) -> &ManifestEntry {
        // `store_contents` rejects a store without manifests.
        &self.manifests[self.manifests.len() - 1]
    }

    /// The manifest labelled `label`.
    pub fn manifest_by_label(&self, label: &str) -> Option<&ManifestEntry> {
        self.manifests.iter().find(|entry| entry.label == label)
    }

    /// Boxes the parser skipped, in the order it met them.
    pub fn ignored(&self) -> &[IgnoredBox] {
        &self.ignored
    }
}

/// Longest label shown in errors and paths, in characters of the label.
const MAX_DISPLAY_CHARS: usize = 64;

/// `label` made safe to print: printable ASCII as is, every other character
/// as `\u{..}`, and at most [`MAX_DISPLAY_CHARS`] characters followed by
/// `...` when longer.
pub(crate) fn display_label(label: &str) -> String {
    let mut out = String::new();
    for (i, c) in label.chars().enumerate() {
        if i == MAX_DISPLAY_CHARS {
            out.push_str("...");
            break;
        }
        if (' '..='~').contains(&c) && c != '\\' {
            out.push(c);
        } else {
            out.extend(c.escape_unicode());
        }
    }
    out
}
