//! C2PA-S-008: the core reads and writes through caller-supplied async byte
//! sources and sinks. These tests drive the in-memory implementations and the
//! bounded `read_exact_at` helper with a single-threaded executor.

use filesquid_c2pa::io::{
    read_exact_at, ByteIoError, ByteSink, ByteSource, MemorySink, MemorySource,
};
use futures::executor::block_on;

/// A source that hands out at most one byte per call, to prove `read_exact_at`
/// keeps reading until the buffer is full.
struct TrickleSource {
    /// Bytes served one at a time.
    bytes: Vec<u8>,
}

impl ByteSource for TrickleSource {
    async fn size(&mut self) -> Result<u64, ByteIoError> {
        Ok(self.bytes.len() as u64)
    }

    async fn read_at(&mut self, offset: u64, buf: &mut [u8]) -> Result<usize, ByteIoError> {
        let start = usize::try_from(offset).expect("offset fits");
        if start >= self.bytes.len() || buf.is_empty() {
            return Ok(0);
        }
        buf[0] = self.bytes[start];
        Ok(1)
    }
}

/// A source whose storage always fails.
struct BrokenSource;

impl ByteSource for BrokenSource {
    async fn size(&mut self) -> Result<u64, ByteIoError> {
        Err(ByteIoError::Backend("disk unplugged".into()))
    }

    async fn read_at(&mut self, _offset: u64, _buf: &mut [u8]) -> Result<usize, ByteIoError> {
        Err(ByteIoError::Backend("disk unplugged".into()))
    }
}

#[test]
fn storage_errors_propagate_through_read_exact_at() {
    let mut buf = [0u8; 4];
    let err = block_on(read_exact_at(&mut BrokenSource, 0, &mut buf)).expect_err("backend");
    assert_eq!(err, ByteIoError::Backend("disk unplugged".into()));
    assert_eq!(
        block_on(BrokenSource.size()),
        Err(ByteIoError::Backend("disk unplugged".into()))
    );
}

/// A source that serves one byte at every offset, even past `u64::MAX - 1`.
struct EndlessSource;

impl ByteSource for EndlessSource {
    async fn size(&mut self) -> Result<u64, ByteIoError> {
        Ok(u64::MAX)
    }

    async fn read_at(&mut self, _offset: u64, buf: &mut [u8]) -> Result<usize, ByteIoError> {
        buf[0] = 1;
        Ok(1)
    }
}

#[test]
fn read_exact_at_reports_offset_overflow() {
    let mut buf = [0u8; 2];
    let err =
        block_on(read_exact_at(&mut EndlessSource, u64::MAX, &mut buf)).expect_err("overflow");
    assert!(matches!(err, ByteIoError::OutOfRange { .. }), "{err:?}");
}

#[test]
fn memory_source_reports_its_size() {
    let mut source = MemorySource::new(b"jumbf".to_vec());
    assert_eq!(block_on(source.size()), Ok(5));
}

#[test]
fn memory_source_reads_a_range() {
    let mut source = MemorySource::new(b"0123456789".to_vec());
    let mut buf = [0u8; 4];
    let read = block_on(source.read_at(3, &mut buf)).expect("read");
    assert_eq!(read, 4);
    assert_eq!(&buf, b"3456");
}

#[test]
fn memory_source_short_read_at_the_end() {
    let mut source = MemorySource::new(b"abc".to_vec());
    let mut buf = [0u8; 8];
    assert_eq!(block_on(source.read_at(1, &mut buf)), Ok(2));
    assert_eq!(&buf[..2], b"bc");
}

#[test]
fn memory_source_returns_zero_at_end_of_data() {
    let mut source = MemorySource::new(b"abc".to_vec());
    let mut buf = [0u8; 2];
    assert_eq!(block_on(source.read_at(3, &mut buf)), Ok(0));
}

#[test]
fn memory_source_rejects_offset_past_the_end() {
    let mut source = MemorySource::new(b"abc".to_vec());
    let mut buf = [0u8; 2];
    let err = block_on(source.read_at(4, &mut buf)).expect_err("out of range");
    assert_eq!(err, ByteIoError::OutOfRange { offset: 4, size: 3 });
}

#[test]
fn read_exact_at_fills_the_buffer_across_short_reads() {
    let mut source = TrickleSource {
        bytes: b"manifest".to_vec(),
    };
    let mut buf = [0u8; 5];
    block_on(read_exact_at(&mut source, 2, &mut buf)).expect("read exact");
    assert_eq!(&buf, b"nifes");
}

#[test]
fn read_exact_at_reports_unexpected_end_of_data() {
    let mut source = MemorySource::new(b"short".to_vec());
    let mut buf = [0u8; 8];
    let err = block_on(read_exact_at(&mut source, 2, &mut buf)).expect_err("eof");
    assert_eq!(
        err,
        ByteIoError::UnexpectedEof {
            offset: 2,
            wanted: 8,
            got: 3
        }
    );
}

#[test]
fn read_exact_at_with_empty_buffer_reads_nothing() {
    let mut source = MemorySource::new(Vec::new());
    let mut buf = [0u8; 0];
    assert_eq!(block_on(read_exact_at(&mut source, 0, &mut buf)), Ok(()));
}

#[test]
fn memory_sink_collects_writes_until_finished() {
    let mut sink = MemorySink::new();
    block_on(sink.write_all(b"c2pa")).expect("write");
    block_on(sink.write_all(b"-box")).expect("write");
    assert!(!sink.is_finished());
    block_on(sink.finish()).expect("finish");
    assert!(sink.is_finished());
    assert_eq!(sink.bytes(), b"c2pa-box");
}

#[test]
fn memory_sink_rejects_writes_after_finish() {
    let mut sink = MemorySink::new();
    block_on(sink.finish()).expect("finish");
    let err = block_on(sink.write_all(b"late")).expect_err("closed");
    assert_eq!(err, ByteIoError::Finished);
    assert_eq!(sink.into_bytes(), Vec::<u8>::new());
}

#[test]
fn memory_sink_finish_twice_is_an_error() {
    let mut sink = MemorySink::new();
    block_on(sink.finish()).expect("finish");
    assert_eq!(block_on(sink.finish()), Err(ByteIoError::Finished));
}
