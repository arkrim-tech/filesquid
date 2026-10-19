//! Assembling the next version outside the object root (ADR 016 write
//! steps 4 to 6): hash the staged bytes, build the new inventory and list
//! the bitstreams to upload. Nothing is written to the storage root here.
//!
//! Only bytes whose digest the manifest does not hold yet are uploaded,
//! once each, to `vN/<contentDirectory>/<first logical path with that
//! digest>`. Unchanged, renamed and reinstated files, and duplicates within
//! the version, point at existing manifest entries.

use std::collections::BTreeMap;

use super::draft::{DraftFile, VersionDraft};
use super::error::CommitError;
use crate::digest::DigestAlgorithm;
use crate::inventory::{DigestMap, Inventory, Version, VersionNum};
use crate::namaste::SpecVersion;
use crate::storage::{
    ChunkReader, StorageBackend, StorageError, StorageKey, MAX_CHUNK_BYTES, MAX_OBJECT_BYTES,
};

/// Most chunks one staged file is read in.
const MAX_HASH_CHUNKS: u64 = MAX_OBJECT_BYTES / MAX_CHUNK_BYTES + 2;

/// Where the bytes of one upload come from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum UploadSource {
    /// Bytes at this key of the caller's staging backend.
    Staged(StorageKey),
    /// Bytes a version-bound file function returned, held in memory (at
    /// most [`MAX_PUT_BYTES`](crate::storage::MAX_PUT_BYTES)).
    Inline(Vec<u8>),
}

/// One bitstream to upload: from its source to the content path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Upload {
    /// Content path relative to the object root, under `vN/`.
    pub content_path: String,
    /// Where the bytes come from.
    pub source: UploadSource,
    /// The bytes' sha256 digest, as the manifest records it.
    pub digest: String,
}

/// The next version, assembled.
#[derive(Debug, Clone)]
pub(crate) struct VersionPlan {
    /// The version to publish.
    pub version: VersionNum,
    /// The new inventory.
    pub inventory: Inventory,
    /// The new inventory's bytes, which the root and `vN` inventories get.
    pub bytes: Vec<u8>,
    /// Bitstreams to upload, in content-path order.
    pub uploads: Vec<Upload>,
}

impl VersionPlan {
    /// The object identifier.
    pub fn object_id(&self) -> &str {
        &self.inventory.id
    }

    /// The upload for `content_path`, if the plan writes it.
    pub fn upload(&self, content_path: &str) -> Option<&Upload> {
        self.uploads.iter().find(|u| u.content_path == content_path)
    }
}

/// Assemble `draft` on top of `base` (`None` for a new object), reading
/// the staged bytes from `staging`.
///
/// # Errors
///
/// [`CommitError::InvalidDraft`] for a draft that cannot be a valid next
/// version, [`CommitError::UnsupportedAlgorithm`] for a base that is not
/// sha256, or a staging backend error.
pub(crate) async fn assemble<S: StorageBackend>(
    base: Option<&Inventory>,
    staging: &S,
    draft: &VersionDraft,
) -> Result<VersionPlan, CommitError> {
    check_base(base, draft)?;
    let version = next_version(base)?;
    let digests = hash_staged(staging, draft).await?;
    let mut inventory = base.cloned().unwrap_or_else(|| new_inventory(draft));
    let state = build_state(&inventory.manifest, draft, &digests)?;
    let uploads = extend_manifest(&mut inventory, &version, &state, draft)?;
    inventory.head = version.clone();
    inventory
        .versions
        .insert(version.clone(), version_block(draft, state));
    let bytes = inventory.to_json();
    Inventory::parse(&bytes)?;
    Ok(VersionPlan {
        version,
        inventory,
        bytes,
        uploads,
    })
}

/// The draft names the base object, `created` is UTC, and the base is
/// sha256.
fn check_base(base: Option<&Inventory>, draft: &VersionDraft) -> Result<(), CommitError> {
    if !draft.created().ends_with('Z') {
        return Err(CommitError::draft(
            "created must be RFC 3339 UTC, ending in Z",
        ));
    }
    let Some(base) = base else {
        return Ok(());
    };
    if base.id != draft.object_id() {
        return Err(CommitError::draft(format!(
            "the draft is for {:?}, the object is {:?}",
            draft.object_id(),
            base.id
        )));
    }
    if base.digest_algorithm != DigestAlgorithm::ENGINE {
        return Err(CommitError::UnsupportedAlgorithm {
            algorithm: base.digest_algorithm.name().to_string(),
        });
    }
    Ok(())
}

/// The version after the base head, or `v1` for a new object.
fn next_version(base: Option<&Inventory>) -> Result<VersionNum, CommitError> {
    let Some(base) = base else {
        return Ok(VersionNum::first());
    };
    base.head
        .next()
        .ok_or_else(|| CommitError::draft("the zero-padded version names are used up"))
}

/// An inventory for a new sha256 object with no versions yet.
fn new_inventory(draft: &VersionDraft) -> Inventory {
    Inventory {
        id: draft.object_id().to_string(),
        spec: SpecVersion::V1_1,
        digest_algorithm: DigestAlgorithm::ENGINE,
        head: VersionNum::first(),
        content_directory: None,
        manifest: BTreeMap::new(),
        versions: BTreeMap::new(),
        fixity: None,
    }
}

/// The sha256 digest of every staged file, by staging key.
async fn hash_staged<S: StorageBackend>(
    staging: &S,
    draft: &VersionDraft,
) -> Result<BTreeMap<StorageKey, String>, CommitError> {
    let mut digests = BTreeMap::new();
    for file in draft.files().values() {
        let DraftFile::Staged(key) = file else {
            continue;
        };
        if !digests.contains_key(key) {
            let digest = hash_one(staging, key).await?;
            digests.insert(key.clone(), digest);
        }
    }
    Ok(digests)
}

/// Stream one staged file in chunks of [`MAX_CHUNK_BYTES`] and hash it.
async fn hash_one<S: StorageBackend>(staging: &S, key: &StorageKey) -> Result<String, CommitError> {
    let mut reader = ChunkReader::new(staging, key.clone(), MAX_CHUNK_BYTES)?;
    let mut hasher = DigestAlgorithm::ENGINE.hasher();
    for _ in 0..MAX_HASH_CHUNKS {
        match reader.next_chunk().await {
            Ok(Some(bytes)) => hasher.update(&bytes),
            Ok(None) => return Ok(hasher.finalize_hex()),
            Err(StorageError::NotFound { .. }) => {
                return Err(CommitError::draft(format!("nothing is staged at {key}")))
            }
            Err(err) => return Err(err.into()),
        }
    }
    Err(StorageError::LimitExceeded {
        what: "chunks read from one staged file",
        limit: MAX_HASH_CHUNKS,
    }
    .into())
}

/// The new version's state: every logical path under its digest. Digests
/// the manifest already holds keep the manifest's spelling, so state and
/// manifest match exactly (E050) and no digest is listed twice (E096).
fn build_state(
    manifest: &DigestMap,
    draft: &VersionDraft,
    digests: &BTreeMap<StorageKey, String>,
) -> Result<DigestMap, CommitError> {
    let mut state = DigestMap::new();
    for (path, file) in draft.files() {
        let digest = match file {
            DraftFile::Staged(key) => {
                let computed = &digests[key];
                manifest_key(manifest, computed).unwrap_or(computed).clone()
            }
            DraftFile::Existing(digest) => manifest_key(manifest, digest)
                .ok_or_else(|| {
                    CommitError::draft(format!("{path}: the manifest has no digest {digest}"))
                })?
                .clone(),
        };
        state.entry(digest).or_default().push(path.clone());
    }
    Ok(state)
}

/// The manifest's own spelling of `digest`, compared without case.
pub(crate) fn manifest_key<'m>(manifest: &'m DigestMap, digest: &str) -> Option<&'m String> {
    manifest.keys().find(|key| key.eq_ignore_ascii_case(digest))
}

/// Add every digest of `state` that the manifest lacks, at a content path
/// under `vN/`, and return the uploads that store their bytes.
fn extend_manifest(
    inventory: &mut Inventory,
    version: &VersionNum,
    state: &DigestMap,
    draft: &VersionDraft,
) -> Result<Vec<Upload>, CommitError> {
    let content_dir = inventory.content_directory().to_string();
    let mut uploads = Vec::new();
    for (digest, paths) in state {
        if inventory.manifest.contains_key(digest) {
            continue;
        }
        let first = &paths[0];
        let DraftFile::Staged(staged) = &draft.files()[first] else {
            return Err(CommitError::draft(format!(
                "{first}: no bytes for {digest}"
            )));
        };
        let content_path = format!("{version}/{content_dir}/{first}");
        // Refuse a path no storage key can name (such as one with `\` or
        // NUL) now, before anything is reserved or written.
        StorageKey::new(&content_path)
            .map_err(|err| CommitError::draft(format!("{first}: {err}")))?;
        inventory
            .manifest
            .insert(digest.clone(), vec![content_path.clone()]);
        uploads.push(Upload {
            content_path,
            source: UploadSource::Staged(staged.clone()),
            digest: digest.clone(),
        });
    }
    uploads.sort_by(|a, b| a.content_path.cmp(&b.content_path));
    Ok(uploads)
}

/// The new version block.
fn version_block(draft: &VersionDraft, state: DigestMap) -> Version {
    Version {
        created: draft.created().to_string(),
        state,
        message: draft.message().map(str::to_string),
        user: draft.user().cloned(),
    }
}
