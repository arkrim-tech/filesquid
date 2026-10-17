//! Caller-supplied async byte sources and sinks (C2PA-S-008).
//!
//! The engine reads assets and writes manifests only through these traits; a
//! host decides whether the bytes come from a file, an OCFL object, object
//! storage, a browser `Blob` or memory. Methods return futures with no `Send`
//! bound so single-threaded `wasm32` hosts can implement them; on native
//! multi-threaded hosts the futures are `Send` whenever the implementation's
//! are.

use std::future::Future;

/// Errors from a byte source or sink.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ByteIoError {
    /// A read started past the end of the source.
    #[error("read offset {offset} is past the end of a {size}-byte source")]
    OutOfRange {
        /// Requested offset.
        offset: u64,
        /// Size of the source.
        size: u64,
    },
    /// The source ended before a buffer could be filled.
    #[error("source ended at offset {offset}: wanted {wanted} bytes, got {got}")]
    UnexpectedEof {
        /// Offset the read started at.
        offset: u64,
        /// Bytes requested.
        wanted: usize,
        /// Bytes available.
        got: usize,
    },
    /// The sink was already finished.
    #[error("the sink is already finished")]
    Finished,
    /// The caller's storage reported an error.
    #[error("byte storage error: {0}")]
    Backend(String),
}

/// Random-access, read-only bytes supplied by the caller.
pub trait ByteSource {
    /// Total size of the source in bytes.
    fn size(&mut self) -> impl Future<Output = Result<u64, ByteIoError>>;

    /// Read up to `buf.len()` bytes starting at `offset` into `buf`.
    ///
    /// Returns the number of bytes read; `0` means end of data. An `offset`
    /// greater than the size is [`ByteIoError::OutOfRange`].
    fn read_at(
        &mut self,
        offset: u64,
        buf: &mut [u8],
    ) -> impl Future<Output = Result<usize, ByteIoError>>;
}

/// Append-only byte destination supplied by the caller.
///
/// The engine calls [`ByteSink::finish`] exactly once after the last write, on
/// success; the caller then commits or closes its storage. A sink that is
/// dropped without `finish` MUST be treated by the caller as incomplete.
pub trait ByteSink {
    /// Append all of `bytes`.
    fn write_all(&mut self, bytes: &[u8]) -> impl Future<Output = Result<(), ByteIoError>>;

    /// Mark the output complete. Writes after this fail with
    /// [`ByteIoError::Finished`].
    fn finish(&mut self) -> impl Future<Output = Result<(), ByteIoError>>;
}

/// Fill `buf` from `source` starting at `offset`, across short reads.
///
/// The loop is bounded: every iteration must read at least one byte, so it
/// runs at most `buf.len()` times; a zero-byte read ends it with
/// [`ByteIoError::UnexpectedEof`].
pub async fn read_exact_at<S: ByteSource + ?Sized>(
    source: &mut S,
    offset: u64,
    buf: &mut [u8],
) -> Result<(), ByteIoError> {
    let wanted = buf.len();
    let mut filled = 0usize;
    while filled < wanted {
        let at = offset
            .checked_add(filled as u64)
            .ok_or(ByteIoError::OutOfRange {
                offset,
                size: u64::MAX,
            })?;
        let read = source.read_at(at, &mut buf[filled..]).await?;
        if read == 0 {
            return Err(ByteIoError::UnexpectedEof {
                offset,
                wanted,
                got: filled,
            });
        }
        let space = wanted - filled;
        if read > space {
            return Err(ByteIoError::Backend(format!(
                "source reported {read} bytes read into a {space}-byte buffer"
            )));
        }
        filled += read;
    }
    Ok(())
}

/// A [`ByteSource`] over bytes held in memory.
#[derive(Debug, Clone, Default)]
pub struct MemorySource {
    /// The source bytes.
    bytes: Vec<u8>,
}

impl MemorySource {
    /// Wrap `bytes` as a source.
    pub fn new(bytes: Vec<u8>) -> Self {
        Self { bytes }
    }
}

impl ByteSource for MemorySource {
    async fn size(&mut self) -> Result<u64, ByteIoError> {
        Ok(self.bytes.len() as u64)
    }

    async fn read_at(&mut self, offset: u64, buf: &mut [u8]) -> Result<usize, ByteIoError> {
        let size = self.bytes.len() as u64;
        if offset > size {
            return Err(ByteIoError::OutOfRange { offset, size });
        }
        // `offset <= size`, and `size` came from a `usize`, so this cannot truncate.
        let start = offset as usize;
        let count = buf.len().min(self.bytes.len() - start);
        buf[..count].copy_from_slice(&self.bytes[start..start + count]);
        Ok(count)
    }
}

/// A [`ByteSink`] that collects bytes in memory.
#[derive(Debug, Clone, Default)]
pub struct MemorySink {
    /// Bytes written so far.
    bytes: Vec<u8>,
    /// Whether [`ByteSink::finish`] was called.
    finished: bool,
}

impl MemorySink {
    /// An empty, unfinished sink.
    pub fn new() -> Self {
        Self::default()
    }

    /// The bytes written so far.
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// Whether the sink was finished.
    pub fn is_finished(&self) -> bool {
        self.finished
    }

    /// Consume the sink and return its bytes.
    pub fn into_bytes(self) -> Vec<u8> {
        self.bytes
    }
}

impl ByteSink for MemorySink {
    async fn write_all(&mut self, bytes: &[u8]) -> Result<(), ByteIoError> {
        if self.finished {
            return Err(ByteIoError::Finished);
        }
        self.bytes.extend_from_slice(bytes);
        Ok(())
    }

    async fn finish(&mut self) -> Result<(), ByteIoError> {
        if self.finished {
            return Err(ByteIoError::Finished);
        }
        self.finished = true;
        Ok(())
    }
}
