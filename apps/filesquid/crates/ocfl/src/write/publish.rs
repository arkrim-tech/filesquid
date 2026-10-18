//! Publishing one reserved version in OCFL write order (OCFL-S-018; ADR 016
//! write step 6): content, then the version inventory and its sidecar, then
//! the root inventory and, last, the root sidecar. Every write is preceded
//! by a fence check, content bytes are re-verified against the digests the
//! plan assembled, and nothing under `vN/` is ever overwritten
//! (`IfAbsent`), so a stale writer cannot replace what the current one
//! wrote.

use std::collections::BTreeMap;

use super::error::CommitError;
use super::fence::Fence;
use super::listing::list_version_keys;
use super::plan::{Upload, VersionPlan};
use super::reconcile::{foreign, relative};
use super::reservation::VersionReservations;
use crate::digest::DigestAlgorithm;
use crate::inventory::{sidecar_contents, sidecar_name, INVENTORY_FILE};
use crate::namaste::{Declaration, SpecVersion};
use crate::read::small::{child, key_in, read_small};
use crate::read::{LogicalFile, LogicalFileReader, ReadError};
use crate::storage::{
    ETag, MultipartWriter, Precondition, StorageBackend, StorageError, StorageKey, MAX_CHUNK_BYTES,
    MAX_OBJECT_BYTES, MAX_PART_BYTES,
};

/// Most chunks one upload reads.
const MAX_UPLOAD_CHUNKS: u64 = MAX_OBJECT_BYTES / MAX_CHUNK_BYTES + 2;

/// Write the object declaration of a new object. One that is already there
/// with the same contents (left by a crashed first commit) is kept.
///
/// # Errors
///
/// [`CommitError::ForeignWrite`] when a different file is there, a fence
/// failure or a backend error.
pub(crate) async fn write_declaration<B: StorageBackend, R: VersionReservations>(
    storage: &B,
    root: &str,
    fence: &Fence<'_, R>,
) -> Result<(), CommitError> {
    let declaration = Declaration::object(SpecVersion::V1_1);
    let key = key_in(root, &declaration.file_name())?;
    let contents = declaration.contents();
    fence.check().await?;
    match storage
        .put_if(&key, contents.as_bytes(), &Precondition::IfAbsent)
        .await
    {
        Ok(_) => return Ok(()),
        Err(StorageError::PreconditionFailed { .. }) => {}
        Err(err) => return Err(err.into()),
    }
    let existing = read_small(storage, &key, contents.len(), "object declaration").await;
    match existing {
        Ok(bytes) if bytes == contents.as_bytes() => Ok(()),
        Ok(_) | Err(ReadError::Format(_)) => Err(CommitError::ForeignWrite {
            key: key.to_string(),
        }),
        Err(err) => Err(err.into()),
    }
}

/// Upload every bitstream of the plan that was not kept. Returns every
/// content path of the plan with its entity tag, and the paths written.
///
/// # Errors
///
/// As [`upload_one`].
pub(crate) async fn upload_content<B, S, R>(
    storage: &B,
    staging: &S,
    root: &str,
    plan: &VersionPlan,
    mut tags: BTreeMap<String, ETag>,
    fence: &Fence<'_, R>,
) -> Result<(BTreeMap<String, ETag>, Vec<String>), CommitError>
where
    B: StorageBackend,
    S: StorageBackend,
    R: VersionReservations,
{
    let mut written = Vec::new();
    for upload in &plan.uploads {
        if tags.contains_key(&upload.content_path) {
            continue;
        }
        let tag = upload_one(storage, staging, root, upload, fence).await?;
        tags.insert(upload.content_path.clone(), tag);
        written.push(upload.content_path.clone());
    }
    Ok((tags, written))
}

/// Stream one staged file into a multipart upload at its content path and
/// publish it only when every byte matched the planned digest, no other
/// object is at the path, and the fence still holds.
///
/// # Errors
///
/// [`CommitError::StagingChanged`] when the staged bytes no longer match,
/// [`CommitError::ForeignWrite`] when the path is taken, a fence failure or
/// a backend error. The upload is aborted on every error.
async fn upload_one<B, S, R>(
    storage: &B,
    staging: &S,
    root: &str,
    upload: &Upload,
    fence: &Fence<'_, R>,
) -> Result<ETag, CommitError>
where
    B: StorageBackend,
    S: StorageBackend,
    R: VersionReservations,
{
    let key = key_in(root, &upload.content_path)?;
    let mut source = staged_reader(staging, upload)?;
    fence.check().await?;
    let mut writer = MultipartWriter::start(storage, key.clone()).await?;
    let sent = send_parts(&mut source, &mut writer, upload, fence).await;
    let published = match sent {
        Ok(()) => writer
            .finish(Some(&Precondition::IfAbsent))
            .await
            .map_err(|err| foreign(err, &key)),
        Err(err) => Err(err),
    };
    if published.is_err() {
        if let Err(abort) = writer.abort().await {
            tracing::warn!(error = %abort, key = %key, "could not abort a content upload");
        }
    }
    published.map(|meta| meta.etag)
}

/// A reader of the staged bytes that checks them against the plan's digest.
fn staged_reader<'s, S: StorageBackend>(
    staging: &'s S,
    upload: &Upload,
) -> Result<LogicalFileReader<'s, S>, CommitError> {
    let file = LogicalFile {
        path: upload.content_path.clone(),
        digest: upload.digest.clone(),
        content_path: upload.staged.as_str().to_string(),
    };
    let algorithm = DigestAlgorithm::ENGINE;
    LogicalFileReader::new(
        staging,
        upload.staged.clone(),
        &file,
        algorithm,
        MAX_CHUNK_BYTES,
    )
    .map_err(CommitError::from)
}

/// Read every chunk of `source` into parts of at most [`MAX_PART_BYTES`],
/// confirming the fence before each part and once more after the last
/// byte matched. Always uploads at least one part, so empty files work.
async fn send_parts<S, B, R>(
    source: &mut LogicalFileReader<'_, S>,
    writer: &mut MultipartWriter<'_, B>,
    upload: &Upload,
    fence: &Fence<'_, R>,
) -> Result<(), CommitError>
where
    S: StorageBackend,
    B: StorageBackend,
    R: VersionReservations,
{
    let mut part = Vec::new();
    let mut parts = 0usize;
    for _ in 0..MAX_UPLOAD_CHUNKS {
        let next = source.next_chunk().await;
        let Some(chunk) = next.map_err(|err| staging_error(err, upload))? else {
            if !part.is_empty() || parts == 0 {
                fence.check().await?;
                writer.write_part(&part).await?;
            }
            return fence.check().await;
        };
        part.extend_from_slice(&chunk);
        if part.len() as u64 + MAX_CHUNK_BYTES > MAX_PART_BYTES {
            fence.check().await?;
            writer.write_part(&part).await?;
            part.clear();
            parts += 1;
        }
    }
    Err(StorageError::LimitExceeded {
        what: "chunks read for one upload",
        limit: MAX_UPLOAD_CHUNKS,
    }
    .into())
}

/// A staged file that no longer matches its planned digest (E092 from the
/// reader) changed after assembly.
fn staging_error(err: ReadError, upload: &Upload) -> CommitError {
    if err.code() == Some("E092") {
        return CommitError::StagingChanged {
            key: upload.staged.as_str().to_string(),
        };
    }
    err.into()
}

/// Write the version inventory and its sidecar under `vN/`.
///
/// # Errors
///
/// [`CommitError::ForeignWrite`] when either is already there, a fence
/// failure or a backend error.
pub(crate) async fn write_version_inventory<B: StorageBackend, R: VersionReservations>(
    storage: &B,
    root: &str,
    plan: &VersionPlan,
    fence: &Fence<'_, R>,
) -> Result<(), CommitError> {
    let dir = child(root, &plan.version.to_string());
    let sidecar = sidecar_contents(DigestAlgorithm::ENGINE, &plan.bytes);
    let files = [
        (INVENTORY_FILE.to_string(), plan.bytes.as_slice()),
        (sidecar_name(DigestAlgorithm::ENGINE), sidecar.as_bytes()),
    ];
    for (name, bytes) in files {
        let key = key_in(&dir, &name)?;
        fence.check().await?;
        storage
            .put_if(&key, bytes, &Precondition::IfAbsent)
            .await
            .map_err(|err| foreign(err, &key))?;
    }
    Ok(())
}

/// Check that `vN/` holds exactly the plan's content files, each still the
/// object written or kept, and the version inventory and sidecar.
///
/// # Errors
///
/// [`CommitError::ForeignWrite`] naming the first unexpected, missing or
/// changed key, or a backend error.
pub(crate) async fn verify_version_dir<B: StorageBackend>(
    storage: &B,
    root: &str,
    plan: &VersionPlan,
    tags: &BTreeMap<String, ETag>,
) -> Result<(), CommitError> {
    let keys = list_version_keys(storage, root, &plan.version).await?;
    let listed: Vec<String> = keys.iter().map(|k| relative(root, k).to_string()).collect();
    let version = plan.version.to_string();
    let mut expected: Vec<String> = tags.keys().cloned().collect();
    expected.push(format!("{version}/{INVENTORY_FILE}"));
    expected.push(format!(
        "{version}/{}",
        sidecar_name(DigestAlgorithm::ENGINE)
    ));
    expected.sort();
    if let Some(odd) = first_difference(&listed, &expected) {
        return Err(CommitError::ForeignWrite {
            key: child(root, odd),
        });
    }
    for (path, tag) in tags {
        let key = key_in(root, path)?;
        if &storage.head(&key).await?.etag != tag {
            return Err(CommitError::ForeignWrite {
                key: key.to_string(),
            });
        }
    }
    Ok(())
}

/// The first entry of `listed` or `expected` that the other lacks.
fn first_difference<'a>(listed: &'a [String], expected: &'a [String]) -> Option<&'a str> {
    let extra = listed.iter().find(|k| expected.binary_search(k).is_err());
    let missing = expected.iter().find(|k| listed.binary_search(k).is_err());
    extra.or(missing).map(String::as_str)
}

/// Publish the head: the root inventory by compare-and-swap on the base,
/// then the root sidecar, which commits the version, by compare-and-swap on
/// the sidecar read before the root inventory was replaced. A writer that
/// lost its reservation after its last fence check therefore cannot
/// overwrite a sidecar another writer published meanwhile.
///
/// # Errors
///
/// [`CommitError::ForeignWrite`] when the root inventory or its sidecar
/// changed since it was read, a fence failure or a backend error.
pub(crate) async fn publish_head<B: StorageBackend, R: VersionReservations>(
    storage: &B,
    root: &str,
    plan: &VersionPlan,
    base_tag: Option<&ETag>,
    fence: &Fence<'_, R>,
) -> Result<(), CommitError> {
    let key = key_in(root, INVENTORY_FILE)?;
    let sidecar_key = key_in(root, &sidecar_name(DigestAlgorithm::ENGINE))?;
    let sidecar_precondition = current_precondition(storage, &sidecar_key).await?;
    fence.check().await?;
    storage
        .put_if(&key, &plan.bytes, &precondition_for(base_tag))
        .await
        .map_err(|err| foreign(err, &key))?;
    let sidecar = sidecar_contents(DigestAlgorithm::ENGINE, &plan.bytes);
    fence.check().await?;
    storage
        .put_if(&sidecar_key, sidecar.as_bytes(), &sidecar_precondition)
        .await
        .map_err(|err| foreign(err, &sidecar_key))?;
    Ok(())
}

/// `IfMatch` the tag, or `IfAbsent` when there is none.
fn precondition_for(tag: Option<&ETag>) -> Precondition {
    match tag {
        Some(tag) => Precondition::IfMatch(tag.clone()),
        None => Precondition::IfAbsent,
    }
}

/// The precondition that `key` is still what it is now.
async fn current_precondition<B: StorageBackend>(
    storage: &B,
    key: &StorageKey,
) -> Result<Precondition, CommitError> {
    match storage.head(key).await {
        Ok(meta) => Ok(precondition_for(Some(&meta.etag))),
        Err(StorageError::NotFound { .. }) => Ok(Precondition::IfAbsent),
        Err(err) => Err(err.into()),
    }
}
