//! [`NodeRef`]: what a navigation node stands for, and its opaque handle (API-S-021).
//!
//! A handle is `n1.` and the base64url text (no padding) of a short record of the node:
//! its kind, its identifier (or, for a folder, its scope identifier and its logical path).
//! Clients MUST NOT parse it. It never holds a filesystem path, storage key, OCFL content
//! path, SQL or token (API-S-005): a folder's logical path is a name inside a collection,
//! and the record is encoded so nothing in a handle reads as one. A handle is at most
//! [`MAX_HANDLE_BYTES`] bytes, so a folder nested too deep for one has none and is not listed.
//!
//! Decoding is strict: a handle decodes only when it is exactly what [`NodeRef::to_handle`]
//! would write for the node it names, so no other spelling of a node is accepted.

use filesquid_core::{CollectionId, LibraryId, RepositoryId, VirtualCollectionId};

/// Longest node handle, in bytes (API-S-021).
pub const MAX_HANDLE_BYTES: usize = 512;

/// Prefix of a version 1 handle.
const HANDLE_PREFIX: &str = "n1.";

/// The base64url alphabet (RFC 4648 section 5).
const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";

/// Where the files that imply a folder live: one collection. Every file has exactly one home
/// collection, so no other scope exists (ADR 041 Decision 3; the "unfiled" scope is retired).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum FolderScope {
    /// The files whose home is this collection.
    Collection(CollectionId),
}

/// One node of the navigation tree.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum NodeRef {
    /// The host's one repository.
    Repository(RepositoryId),
    /// A library.
    Library(LibraryId),
    /// A collection.
    Collection(CollectionId),
    /// A virtual collection (it has no children here).
    VirtualCollection(VirtualCollectionId),
    /// The repository's one Recently deleted node: a navigation filter over the deleted files
    /// of every library this host has opened, never a storage location (it has no children
    /// here; ADR 041 Decision 8 as amended 2026-10-23).
    RecentlyDeleted(RepositoryId),
    /// A folder implied by logical paths.
    Folder {
        /// Where the files that imply it live.
        scope: FolderScope,
        /// Its logical path: segments joined by `/`, with no empty, `.` or `..` segment.
        path: String,
    },
}

impl NodeRef {
    /// The folder `path` of `scope`.
    pub fn folder(scope: FolderScope, path: impl Into<String>) -> Self {
        Self::Folder {
            scope,
            path: path.into(),
        }
    }

    /// The opaque handle of this node, or `None` when it has none: a folder whose logical
    /// path is not a valid path, or whose handle would pass [`MAX_HANDLE_BYTES`].
    pub fn to_handle(&self) -> Option<String> {
        let record = match self {
            Self::Repository(id) => field('r', id.as_str()),
            Self::Library(id) => field('l', id.as_str()),
            Self::Collection(id) => field('c', id.as_str()),
            Self::VirtualCollection(id) => field('v', id.as_str()),
            Self::RecentlyDeleted(id) => field('d', id.as_str()),
            Self::Folder { scope, path } => folder_record(scope, path)?,
        };
        let handle = format!("{HANDLE_PREFIX}{}", base64url(record.as_bytes()));
        (handle.len() <= MAX_HANDLE_BYTES).then_some(handle)
    }

    /// The node a handle names, or `None` for anything [`Self::to_handle`] did not write.
    pub fn from_handle(handle: &str) -> Option<Self> {
        if handle.len() > MAX_HANDLE_BYTES {
            return None;
        }
        let record = from_base64url(handle.strip_prefix(HANDLE_PREFIX)?)?;
        let node = parse_record(&record)?;
        (node.to_handle().as_deref() == Some(handle)).then_some(node)
    }
}

/// A record field: its tag, the identifier's length in bytes, `:` and the identifier.
fn field(tag: char, id: &str) -> String {
    format!("{tag}{}:{id}", id.len())
}

/// The record of a folder: `f`, the scope tag and identifier, then the logical path.
fn folder_record(scope: &FolderScope, path: &str) -> Option<String> {
    if !is_logical_path(path) {
        return None;
    }
    let scoped = match scope {
        FolderScope::Collection(id) => field('c', id.as_str()),
    };
    Some(format!("f{scoped}{path}"))
}

/// True for a folder's logical path: non-empty segments joined by `/`, none of them `.` or
/// `..`, with no control character.
pub(crate) fn is_logical_path(path: &str) -> bool {
    !path.is_empty()
        && !path.chars().any(char::is_control)
        && path
            .split('/')
            .all(|segment| !segment.is_empty() && segment != "." && segment != "..")
}

/// The node a decoded record describes.
fn parse_record(record: &str) -> Option<NodeRef> {
    let mut chars = record.chars();
    let tag = chars.next()?;
    let rest = chars.as_str();
    if tag == 'f' {
        return parse_folder(rest);
    }
    let (id, tail) = split_id(rest)?;
    if !tail.is_empty() {
        return None;
    }
    match tag {
        'r' => RepositoryId::new(id).ok().map(NodeRef::Repository),
        'l' => LibraryId::new(id).ok().map(NodeRef::Library),
        'c' => CollectionId::new(id).ok().map(NodeRef::Collection),
        'v' => VirtualCollectionId::new(id)
            .ok()
            .map(NodeRef::VirtualCollection),
        'd' => RepositoryId::new(id).ok().map(NodeRef::RecentlyDeleted),
        _ => None,
    }
}

/// The folder a record's tail after `f` describes.
fn parse_folder(rest: &str) -> Option<NodeRef> {
    let mut chars = rest.chars();
    let tag = chars.next()?;
    let (id, path) = split_id(chars.as_str())?;
    let scope = match tag {
        'c' => FolderScope::Collection(CollectionId::new(id).ok()?),
        _ => return None,
    };
    is_logical_path(path).then(|| NodeRef::folder(scope, path))
}

/// Split `<len>:<id><tail>` into the identifier and the tail.
fn split_id(text: &str) -> Option<(&str, &str)> {
    let (len, rest) = text.split_once(':')?;
    let len: usize = len
        .parse()
        .ok()
        .filter(|_| len.bytes().all(|b| b.is_ascii_digit()))?;
    let id = rest.get(..len)?;
    Some((id, rest.get(len..)?))
}

/// `bytes` as base64url text without padding.
fn base64url(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let n = chunk
            .iter()
            .enumerate()
            .fold(0u32, |n, (i, b)| n | (u32::from(*b) << (16 - 8 * i)));
        for i in 0..=chunk.len() {
            out.push(char::from(ALPHABET[((n >> (18 - 6 * i)) & 63) as usize]));
        }
    }
    out
}

/// The text of base64url `text` without padding, or `None` for a character outside the
/// alphabet, an impossible length or bytes that are not UTF-8.
fn from_base64url(text: &str) -> Option<String> {
    if text.len() % 4 == 1 {
        return None;
    }
    let mut bytes = Vec::with_capacity(text.len() / 4 * 3 + 3);
    for chunk in text.as_bytes().chunks(4) {
        let mut n = 0u32;
        for (i, symbol) in chunk.iter().enumerate() {
            let value = ALPHABET.iter().position(|a| a == symbol)? as u32;
            n |= value << (18 - 6 * i);
        }
        let produced = chunk.len() - 1;
        bytes.extend((0..produced).map(|i| ((n >> (16 - 8 * i)) & 255) as u8));
    }
    String::from_utf8(bytes).ok()
}
