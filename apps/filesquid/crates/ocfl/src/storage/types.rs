//! Value types and bounds shared by every storage backend, plus the pure
//! checks (preconditions, part order, paging) that backends apply the same way.

use sha2::{Digest, Sha256};

use super::error::StorageError;
use super::key::{validate_prefix, StorageKey};

/// Largest ranged read: 8 MiB. Logical files are
/// streamed in chunks no larger than this (ADR 025: no API may need a whole
/// large file in memory).
pub const MAX_CHUNK_BYTES: u64 = 8 * 1024 * 1024;

/// Largest single [`put`](super::StorageBackend::put): 8 MiB. Larger objects
/// use multipart upload.
pub const MAX_PUT_BYTES: u64 = 8 * 1024 * 1024;

/// Most parts in one multipart upload: 10 000, the S3 limit.
pub const MAX_PARTS: u32 = 10_000;

/// Most keys in one listing page: 1 000, the S3 limit.
pub const MAX_LIST_PAGE: usize = 1_000;

/// Deepest directory level a filesystem listing walks.
pub const MAX_WALK_DEPTH: usize = 64;

/// Most directory entries one filesystem listing visits.
pub const MAX_WALK_ENTRIES: usize = 1_000_000;

/// Largest multipart part: 64 MiB. Within S3's 5 MiB to 5 GiB part range and
/// small enough to hold in a `wasm32` worker. Reads stay at
/// [`MAX_CHUNK_BYTES`].
pub const MAX_PART_BYTES: u64 = 64 * 1024 * 1024;

/// Largest object a backend stores or a reader streams: [`MAX_PARTS`] parts
/// of [`MAX_PART_BYTES`], 625 GiB. This is the largest object a multipart
/// upload can produce under these bounds; S3 itself allows up to 5 TiB.
pub const MAX_OBJECT_BYTES: u64 = MAX_PARTS as u64 * MAX_PART_BYTES;

/// An opaque entity tag that changes whenever an object's bytes change.
///
/// The shipped backends use the lowercase hexadecimal SHA-256 of the stored
/// bytes; other backends MAY use their own tag (for example an S3 `ETag`).
/// Tags are only compared for equality.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ETag(String);

impl ETag {
    /// Wrap a backend-issued tag.
    pub fn new(tag: impl Into<String>) -> Self {
        Self(tag.into())
    }

    /// The tag the shipped backends give `bytes`: lowercase hex SHA-256.
    pub fn sha256_of(bytes: &[u8]) -> Self {
        Self::from_hasher(Sha256::new_with_prefix(bytes))
    }

    /// Finish an incremental SHA-256 and return it as a tag.
    pub(crate) fn from_hasher(hasher: Sha256) -> Self {
        let digest = hasher.finalize();
        let mut hex = String::with_capacity(digest.len() * 2);
        for byte in digest.iter() {
            hex.push(char::from(HEX[usize::from(byte >> 4)]));
            hex.push(char::from(HEX[usize::from(byte & 0x0f)]));
        }
        Self(hex)
    }

    /// The tag text.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Lowercase hexadecimal digits.
const HEX: &[u8; 16] = b"0123456789abcdef";

/// Size and entity tag of a stored object.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObjectMeta {
    /// Size in bytes.
    pub size: u64,
    /// Entity tag of the current bytes.
    pub etag: ETag,
}

/// A compare-and-swap condition for a conditional write (ADR 012 conflict
/// guard). A write whose condition fails returns
/// [`StorageError::PreconditionFailed`] and changes nothing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Precondition {
    /// Write only when no object exists at the key (create, reserve).
    IfAbsent,
    /// Write only when the current object has this entity tag (head swap).
    IfMatch(ETag),
}

/// A bounded byte range: `len` bytes from offset `start`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ByteRange {
    start: u64,
    len: u64,
}

impl ByteRange {
    /// Build a range of `len` bytes from `start`.
    ///
    /// # Errors
    ///
    /// [`StorageError::InvalidArgument`] when `len` is zero or the range
    /// overflows `u64`; [`StorageError::LimitExceeded`] when `len` is above
    /// [`MAX_CHUNK_BYTES`].
    pub fn new(start: u64, len: u64) -> Result<Self, StorageError> {
        if len == 0 || start.checked_add(len).is_none() {
            return Err(StorageError::InvalidArgument {
                reason: format!("byte range {start}+{len} is empty or overflows"),
            });
        }
        if len > MAX_CHUNK_BYTES {
            return Err(StorageError::LimitExceeded {
                what: "byte range length",
                limit: MAX_CHUNK_BYTES,
            });
        }
        Ok(Self { start, len })
    }

    /// First byte offset.
    pub fn start(&self) -> u64 {
        self.start
    }

    /// Number of bytes requested.
    pub fn len(&self) -> u64 {
        self.len
    }

    /// Always `false`: a range holds at least one byte.
    pub fn is_empty(&self) -> bool {
        false
    }
}

/// One page of a listing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListPage {
    /// Keys on this page, in lexical byte order.
    pub keys: Vec<StorageKey>,
    /// Pass this as `start_after` to fetch the next page; `None` on the last.
    pub next: Option<StorageKey>,
}

/// Identifier of an open multipart upload.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct UploadId(String);

impl UploadId {
    /// Wrap a backend-issued upload identifier.
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    /// The identifier text.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Receipt for one uploaded part, passed back to
/// [`complete_multipart`](super::StorageBackend::complete_multipart).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompletedPart {
    /// 1-based part number.
    pub part_number: u32,
    /// Entity tag of the part's bytes.
    pub etag: ETag,
}

/// Check a precondition against the current tag (`None`: no object).
///
/// # Errors
///
/// [`StorageError::PreconditionFailed`] when the condition does not hold.
pub(crate) fn check_precondition(
    key: &StorageKey,
    current: Option<&ETag>,
    precondition: &Precondition,
) -> Result<(), StorageError> {
    let holds = match precondition {
        Precondition::IfAbsent => current.is_none(),
        Precondition::IfMatch(expected) => current == Some(expected),
    };
    if holds {
        return Ok(());
    }
    Err(StorageError::PreconditionFailed {
        key: key.as_str().to_string(),
    })
}

/// Reject a single `put` above [`MAX_PUT_BYTES`].
pub(crate) fn check_put_size(bytes: &[u8]) -> Result<(), StorageError> {
    if bytes.len() as u64 > MAX_PUT_BYTES {
        return Err(StorageError::LimitExceeded {
            what: "single put size",
            limit: MAX_PUT_BYTES,
        });
    }
    Ok(())
}

/// Reject a part number outside `1..=MAX_PARTS` or a part above
/// [`MAX_PART_BYTES`].
pub(crate) fn check_part(part_number: u32, bytes: &[u8]) -> Result<(), StorageError> {
    if part_number == 0 || part_number > MAX_PARTS {
        return Err(StorageError::InvalidArgument {
            reason: format!("part number {part_number} is outside 1..={MAX_PARTS}"),
        });
    }
    if bytes.len() as u64 > MAX_PART_BYTES {
        return Err(StorageError::LimitExceeded {
            what: "multipart part size",
            limit: MAX_PART_BYTES,
        });
    }
    Ok(())
}

/// Require a non-empty part list in strictly ascending part-number order.
pub(crate) fn check_part_order(parts: &[CompletedPart]) -> Result<(), StorageError> {
    let ascending = parts
        .windows(2)
        .all(|w| w[0].part_number < w[1].part_number);
    if parts.is_empty() || !ascending {
        return Err(StorageError::InvalidArgument {
            reason: "completed parts must be non-empty and in ascending order".to_string(),
        });
    }
    Ok(())
}

/// Validate a listing request's prefix and page size.
pub(crate) fn check_list_request(prefix: &str, limit: usize) -> Result<(), StorageError> {
    validate_prefix(prefix)?;
    if limit == 0 {
        return Err(StorageError::InvalidArgument {
            reason: "page size must be at least 1".to_string(),
        });
    }
    if limit > MAX_LIST_PAGE {
        return Err(StorageError::LimitExceeded {
            what: "listing page size",
            limit: MAX_LIST_PAGE as u64,
        });
    }
    Ok(())
}

/// Cut one page from keys already sorted in lexical byte order: keys under
/// `prefix`, strictly after `start_after`, at most `limit` of them.
pub(crate) fn paginate<'k>(
    sorted: impl Iterator<Item = &'k str>,
    prefix: &str,
    start_after: Option<&StorageKey>,
    limit: usize,
) -> Result<ListPage, StorageError> {
    let after = start_after.map(StorageKey::as_str);
    let mut keys = Vec::with_capacity(limit);
    let mut more = false;
    let selected = sorted
        .filter(|k| k.starts_with(prefix))
        .filter(|k| after.is_none_or(|a| *k > a));
    for raw in selected {
        if keys.len() == limit {
            more = true;
            break;
        }
        keys.push(StorageKey::new(raw)?);
    }
    let next = if more { keys.last().cloned() } else { None };
    Ok(ListPage { keys, next })
}
