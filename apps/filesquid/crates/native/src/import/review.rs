//! The review of an import before it starts (Task DE-1.8; API-S-036 **Review**, owner answer 5
//! of 2026-10-22, verbatim: "Keep the review"): what the files the owner picked would do to
//! the library, worked out without staging a byte.
//!
//! [`review`] lists the picked sources as the job would ([`super::job::list_plan`]), makes the
//! manifest the job will run (each file with its minted file ID and its collection logical
//! path), reads every file once to hash it, and answers the facts the owner reviews: the files
//! found, the ones the library already holds (by SHA-256, against the digests of the imports
//! committed before, [`SqliteJobItems::holds_digest`]), the bytes to copy, the picked names that
//! will be numbered beside a taken path, and the free bytes after the copy. Its plan items
//! carry base names only. [`fingerprint`] identifies the listing by each entry's path, size and
//! modification time, so a start can refuse a source that changed since its review.

use super::index::ImportIndex;
use super::items::SqliteJobItems;
use super::job::{list_plan, manifest_of, Listed, STAGING_CAP_BYTES};
use super::plans::{ImportPlan, ManifestEntry};
use filesquid_api::imports::{PlanFacts, PlanItem};
use sha2::Digest as _;
use std::io::Read;
use std::path::Path;

/// Bytes read at a time while a file is hashed for its review.
const HASH_CHUNK: usize = 8 * 1024 * 1024;

/// What the review of one plan found.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Reviewed {
    /// The manifest the job will run if the plan is started.
    pub manifest: Vec<ManifestEntry>,
    /// The manifest indexes of the files the library already holds.
    pub present: Vec<usize>,
    /// The facts the owner reviews.
    pub facts: PlanFacts,
    /// The plan's files, in the manifest's order, by base name.
    pub items: Vec<PlanItem>,
    /// The fingerprint of the listing.
    pub fingerprint: String,
}

/// One file of the manifest as the review saw it.
struct Seen {
    index: usize,
    file_id: String,
    name: String,
    bytes: u64,
    present: bool,
    numbered: bool,
}

/// Review `plan`: list its sources, make its manifest against `index`, and hash every file to
/// tell which ones the library already holds (`digests`); `free_bytes` is the free space of
/// the library's volume when the host can read it. Reads the filesystem (every picked file
/// once), the index and the digests; stages nothing.
///
/// # Errors
/// The listing's or the manifest's code (`too_many_files`, `destination_required`,
/// `unavailable`).
pub fn review(
    plan: &ImportPlan,
    index: &dyn ImportIndex,
    digests: &SqliteJobItems,
    free_bytes: Option<u64>,
) -> Result<Reviewed, &'static str> {
    let listed = list_plan(plan)?;
    let fingerprint = fingerprint(&listed);
    let manifest = manifest_of(listed, plan, index)?;
    let roots = numbered_sources(plan, &manifest);
    let seen = manifest
        .iter()
        .enumerate()
        .filter_map(|(at, entry)| seen_file(plan, digests, &roots, (at, entry)))
        .collect::<Result<Vec<_>, _>>()?;
    let numbered = roots.iter().filter(|numbered| **numbered).count() as u64;
    let facts = facts_of(plan, &seen, (numbered, free_bytes));
    Ok(Reviewed {
        present: seen.iter().filter(|s| s.present).map(|s| s.index).collect(),
        items: seen.iter().map(plan_item).collect(),
        manifest,
        facts,
        fingerprint,
    })
}

/// The review of one manifest entry: a file with its size, whether the library holds its
/// digest and whether its source is numbered; `None` for an entry that could not be listed.
/// Reads the file and the digests.
fn seen_file(
    plan: &ImportPlan,
    digests: &SqliteJobItems,
    numbered: &[bool],
    (index, entry): (usize, &ManifestEntry),
) -> Option<Result<Seen, &'static str>> {
    let ManifestEntry::File {
        path,
        source,
        file_id,
        ..
    } = entry
    else {
        return None;
    };
    let bytes = std::fs::metadata(path).map_or(0, |meta| meta.len());
    let present = match hash_file(path) {
        Some(digest) => digests.holds_digest(&plan.library_id, &digest),
        None => Ok(false),
    };
    Some(present.map_err(|_| "unavailable").map(|present| {
        Seen {
            index,
            file_id: file_id.clone(),
            name: path
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_default(),
            bytes,
            present,
            numbered: numbered.get(*source).copied().unwrap_or(false),
        }
    }))
}

/// The SHA-256 of the file at `path`, read in chunks of [`HASH_CHUNK`]; `None` when it cannot
/// be read (the job then reports the file's failure). Reads the file once.
pub fn hash_file(path: &Path) -> Option<String> {
    let mut file = std::fs::File::open(path).ok()?;
    let mut hasher = sha2::Sha256::new();
    let mut buffer = vec![0u8; HASH_CHUNK];
    let chunks = 1 + STAGING_CAP_BYTES / HASH_CHUNK as u64;
    for _ in 0..chunks {
        let read = file.read(&mut buffer).ok()?;
        if read == 0 {
            return Some(hex(&hasher.finalize()));
        }
        hasher.update(&buffer[..read]);
    }
    None
}

/// Lowercase hexadecimal. Pure.
fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// For each picked source, whether its name in the destination is numbered beside a taken
/// path: the first segment of its files' logical paths below the destination folder is not
/// the source's own name. Pure.
fn numbered_sources(plan: &ImportPlan, manifest: &[ManifestEntry]) -> Vec<bool> {
    let below = |logical: &str| -> String {
        let rest = match plan.folder.as_deref() {
            Some(folder) => logical.strip_prefix(folder).unwrap_or(logical),
            None => logical,
        };
        rest.trim_start_matches('/')
            .split('/')
            .next()
            .unwrap_or_default()
            .to_string()
    };
    plan.sources
        .iter()
        .enumerate()
        .map(|(at, source)| {
            let name = source
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_default();
            manifest.iter().any(|entry| match entry {
                ManifestEntry::File {
                    source,
                    logical_path: Some(logical),
                    ..
                } if *source == at => below(logical) != name,
                _ => false,
            })
        })
        .collect()
}

/// The facts of `seen` for `plan`, `numbered` of whose sources are numbered, with the free
/// bytes after copying the new files. Reads the source volume's facts.
fn facts_of(
    plan: &ImportPlan,
    seen: &[Seen],
    (numbered, free_bytes): (u64, Option<u64>),
) -> PlanFacts {
    let new: Vec<&Seen> = seen.iter().filter(|s| !s.present).collect();
    let bytes_to_copy = new.iter().map(|s| s.bytes).sum::<u64>();
    PlanFacts {
        source_volume: plan.sources.first().and_then(|source| volume_label(source)),
        files_found: seen.len() as u64,
        files_new: new.len() as u64,
        files_present: (seen.len() - new.len()) as u64,
        bytes_to_copy,
        names_numbered: numbered,
        free_bytes_after: free_bytes.map(|free| free.saturating_sub(bytes_to_copy)),
        staging_limit_bytes: STAGING_CAP_BYTES,
    }
}

/// The label of the volume `path` is on, as its last name segment (never a path); `None` when
/// the host cannot read it. Reads the volume's facts.
fn volume_label(path: &Path) -> Option<String> {
    let volume = super::volume::gather_volume(path);
    let raw = volume.name.as_value()?;
    let text = String::from_utf8_lossy(raw.as_bytes()).into_owned();
    let label = text.rsplit('/').find(|segment| !segment.is_empty())?;
    Some(label.to_string())
}

/// The plan item of one reviewed file. Pure.
fn plan_item(seen: &Seen) -> PlanItem {
    PlanItem {
        item_id: seen.file_id.clone(),
        name: seen.name.clone(),
        bytes: seen.bytes,
        present: seen.present,
        numbered: seen.numbered,
    }
}

/// The fingerprint of `listed`: a SHA-256 over each entry's path, size and modification time
/// (and each unlistable entry's code), in the listing's order. Reads each file's metadata.
pub fn fingerprint(listed: &[Listed]) -> String {
    let mut hasher = sha2::Sha256::new();
    for entry in listed {
        match entry {
            Listed::File { path, source } => {
                let meta = std::fs::metadata(path).ok();
                let size = meta.as_ref().map_or(0, std::fs::Metadata::len);
                let modified = meta
                    .and_then(|meta| meta.modified().ok())
                    .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
                    .map_or(0, |since| since.as_nanos());
                hasher.update(path.as_os_str().as_encoded_bytes());
                hasher.update(format!("\0{source}\0{size}\0{modified}\n").as_bytes());
            }
            Listed::Skipped { source, code } => {
                hasher.update(format!("skipped\0{source}\0{code}\n").as_bytes());
            }
        }
    }
    hex(&hasher.finalize())
}

/// The fingerprint of `plan`'s sources now. Reads the filesystem.
///
/// # Errors
/// `too_many_files` for an over-large selection.
pub fn fingerprint_of_plan(plan: &ImportPlan) -> Result<String, &'static str> {
    list_plan(plan).map(|listed| fingerprint(&listed))
}
