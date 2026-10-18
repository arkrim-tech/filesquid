//! Versioned library queries and paged answers (API-S-019, CORE-S-005).
//!
//! A [`QueryRequest`] is a `filesquid-core` [`QueryEnvelope`] (schema version, scope, page
//! size 1 to 1,000) plus an optional [`Cursor`] and one [`LibraryQuery`]. Every listing
//! answers a [`Page`] in a stable order; `next_cursor` is absent on the last page. A cursor
//! is opaque to callers: a versioned hex encoding of the last item's position, never a path.

use super::catalog::{FileEntry, FileScope, LibraryRecord, LibraryState, PageKey};
use super::policy::{DeletionPolicy, VisibilityFilter};
use super::LibraryError;
use crate::operation::QueryEnvelope;
use filesquid_core::{Collection, LibraryId, RepositoryId, VirtualCollection};
use serde::{Deserialize, Serialize};

/// Prefix of a version 1 cursor.
const CURSOR_PREFIX: &str = "v1.";

/// Longest cursor accepted, in bytes (bounds the decode work).
const MAX_CURSOR_BYTES: usize = 1024;

/// One library query. Serialized with its kind in the `query` member, in `snake_case`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "query", rename_all = "snake_case")]
pub enum LibraryQuery {
    /// Libraries of a repository, in creation order.
    ListLibraries {
        /// The repository.
        repository_id: RepositoryId,
    },
    /// Open one library: its record and current revision.
    OpenLibrary {
        /// The library.
        library_id: LibraryId,
    },
    /// Collections of a library.
    ListCollections {
        /// The library.
        library_id: LibraryId,
    },
    /// Virtual collections of a library.
    ListVirtualCollections {
        /// The library.
        library_id: LibraryId,
    },
    /// Files of a library, collection, virtual collection or the unfiled files, in one view.
    ListFiles {
        /// The library.
        library_id: LibraryId,
        /// Which files.
        scope: FileScope,
        /// Ordinary (default), hidden or deleted view.
        #[serde(default)]
        visibility: VisibilityFilter,
    },
    /// The deletion policy (days a deleted file stays recoverable), shown before deletion.
    DeletionPolicy,
}

/// A versioned library query.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct QueryRequest {
    /// Schema version, scope and bounded page size.
    pub envelope: QueryEnvelope,
    /// Position after which the page starts; absent for the first page.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cursor: Option<Cursor>,
    /// The query.
    pub query: LibraryQuery,
}

/// One page of a listing.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Page<T> {
    /// The items, in listing order.
    pub items: Vec<T>,
    /// Cursor of the next page; absent on the last page.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<Cursor>,
}

/// The answer to a [`LibraryQuery`], tagged with its kind in `answer` and its body in `data`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "answer", content = "data", rename_all = "snake_case")]
pub enum QueryAnswer {
    /// Answer to [`LibraryQuery::ListLibraries`].
    Libraries(Page<LibraryRecord>),
    /// Answer to [`LibraryQuery::OpenLibrary`].
    Library(LibraryState),
    /// Answer to [`LibraryQuery::ListCollections`].
    Collections(Page<Collection>),
    /// Answer to [`LibraryQuery::ListVirtualCollections`].
    VirtualCollections(Page<VirtualCollection>),
    /// Answer to [`LibraryQuery::ListFiles`].
    Files(Page<FileEntry>),
    /// Answer to [`LibraryQuery::DeletionPolicy`].
    DeletionPolicy(DeletionPolicy),
}

/// Opaque paging position. Callers pass it back unchanged.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Cursor(String);

impl Cursor {
    /// Encode `key` as an opaque version 1 cursor: the prefix, then the lowercase hex of
    /// `<order>:<id>`. Every cursor this answers is one [`Self::decode`] accepts.
    ///
    /// # Errors
    ///
    /// Returns [`LibraryError::Internal`] when the cursor would exceed the
    /// `MAX_CURSOR_BYTES` bound that [`Self::decode`] enforces (an item ID of about 490
    /// bytes or more); the detail goes to stdout only.
    pub fn encode(key: &PageKey) -> Result<Self, LibraryError> {
        let plain = format!("{}:{}", key.order, key.id);
        let hex: String = plain.bytes().map(|b| format!("{b:02x}")).collect();
        let text = format!("{CURSOR_PREFIX}{hex}");
        if text.len() > MAX_CURSOR_BYTES {
            tracing::info!(
                target: super::LOG_TARGET,
                error_code = "internal",
                id_bytes = key.id.len(),
                "page key too long for a cursor"
            );
            return Err(LibraryError::Internal);
        }
        Ok(Self(text))
    }

    /// The cursor text.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Decode the position this cursor names.
    ///
    /// # Errors
    ///
    /// Returns an `invalid_cursor` [`LibraryError`] for any text [`Self::encode`] did not
    /// produce, including other spellings of a valid position (uppercase hex, a `+` sign or
    /// leading zeros): the decoded key is encoded again and must give the same text.
    pub fn decode(&self) -> Result<PageKey, LibraryError> {
        let key = self.parse().ok_or_else(invalid_cursor)?;
        match Self::encode(&key) {
            Ok(canonical) if canonical == *self => Ok(key),
            _ => Err(invalid_cursor()),
        }
    }

    /// The position this cursor's text spells, without the canonical-form check.
    fn parse(&self) -> Option<PageKey> {
        let hex = self
            .0
            .strip_prefix(CURSOR_PREFIX)
            .filter(|hex| self.0.len() <= MAX_CURSOR_BYTES && !hex.is_empty())?;
        let plain = decode_hex(hex)?;
        let (order, id) = plain.split_once(':')?;
        let order = order.parse::<i64>().ok()?;
        (!id.is_empty()).then(|| PageKey {
            order,
            id: id.to_string(),
        })
    }
}

/// The error for a cursor this API did not issue.
fn invalid_cursor() -> LibraryError {
    LibraryError::invalid(
        "invalid_cursor",
        "The paging position is not valid. Start again.",
    )
}

/// Decode hex pairs into UTF-8 text (case is checked by [`Cursor::decode`]).
fn decode_hex(hex: &str) -> Option<String> {
    if !hex.len().is_multiple_of(2) {
        return None;
    }
    let bytes: Option<Vec<u8>> = (0..hex.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(hex.get(i..i + 2)?, 16).ok())
        .collect();
    String::from_utf8(bytes?).ok()
}
