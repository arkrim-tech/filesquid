//! Storage backends: the only way the engine reaches bytes (OCFL-S-007,
//! [ADR 025](../../../../resources/architecture/025-client-side-filesquid-engines.md)
//! Decision 2).
//!
//! The host chooses a backend and hands it to the engine; OCFL rules do not
//! change with the backend. This crate ships:
//!
//! - [`MemoryBackend`]: in-memory, every target, for tests.
//! - [`FsBackend`]: local filesystem, native only, behind the default-on `fs`
//!   feature.
//!
//! The S3-compatible backend (native and `wasm32`) implements the same
//! [`StorageBackend`] trait in a later task (IO-5.1).
//!
//! Every transfer is bounded: see [`MAX_CHUNK_BYTES`], [`MAX_PUT_BYTES`],
//! [`MAX_PARTS`] and the streaming helpers [`ChunkReader`] and
//! [`MultipartWriter`].

mod backend;
mod drive;
mod error;
#[cfg(all(feature = "fs", not(target_arch = "wasm32")))]
mod fs;
mod key;
mod memory;
mod stream;
mod types;

pub use backend::{MaybeSend, MaybeSync, StorageBackend};
pub use drive::complete_immediately;
pub use error::StorageError;
#[cfg(all(feature = "fs", not(target_arch = "wasm32")))]
pub use fs::FsBackend;
pub use key::StorageKey;
pub use memory::MemoryBackend;
pub use stream::{ChunkReader, MultipartWriter};
pub use types::{
    ByteRange, CompletedPart, ETag, ListPage, ObjectMeta, Precondition, UploadId, MAX_CHUNK_BYTES,
    MAX_LIST_PAGE, MAX_OBJECT_BYTES, MAX_PARTS, MAX_PART_BYTES, MAX_PUT_BYTES, MAX_WALK_DEPTH,
    MAX_WALK_ENTRIES,
};
