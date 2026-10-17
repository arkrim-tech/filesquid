//! C2PA-S-001 / C2PA-S-008: a rejected manifest store is logged with a fixed
//! error category only, never a label or URL from the input.
//!
//! This test has its own binary: `tracing` caches whether a log call site is
//! enabled, and another test thread hitting the same call site with no
//! subscriber at the same moment could make this capture miss the event.

#[path = "../support/fixtures.rs"]
#[allow(dead_code)]
mod fixtures;

use std::cell::RefCell;
use std::io::Write;
use std::sync::{Arc, Mutex};

use filesquid_c2pa::fetch::{FetchError, FetchRequest, FetchResponse, Fetcher};
use filesquid_c2pa::io::MemorySource;
use filesquid_c2pa::manifest::ParseLimits;
use filesquid_c2pa::read::{fetch_remote_manifest_store, read_manifest_store};
use fixtures::*;
use futures::executor::block_on;

/// A fetcher that must never be called.
struct UnusedFetcher(RefCell<usize>);

impl Fetcher for UnusedFetcher {
    async fn fetch(&self, _request: &FetchRequest) -> Result<FetchResponse, FetchError> {
        *self.0.borrow_mut() += 1;
        Ok(FetchResponse {
            content_type: None,
            body: Vec::new(),
        })
    }
}

/// A `tracing` writer that appends to a shared buffer.
#[derive(Clone, Default)]
struct Captured(Arc<Mutex<Vec<u8>>>);

impl Write for Captured {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0.lock().expect("log buffer").extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

#[test]
fn rejection_logs_carry_categories_only() {
    let captured = Captured::default();
    let writer = captured.clone();
    let subscriber = tracing_subscriber::fmt()
        .with_writer(move || writer.clone())
        .with_ansi(false)
        .finish();
    let hostile = "urn:c2pa:SECRET-LABEL\u{202e}";
    let bytes = store(&[standard_manifest(hostile), standard_manifest(hostile)]);
    tracing::subscriber::with_default(subscriber, || {
        let mut source = MemorySource::new(bytes);
        block_on(read_manifest_store(&mut source, &ParseLimits::default())).expect_err("dup");
        let fetcher = UnusedFetcher(RefCell::new(0));
        block_on(fetch_remote_manifest_store(
            &fetcher,
            "https://e.org/SECRET-PATH#jumbf=c2pa",
            &ParseLimits::default(),
        ))
        .expect_err("embedded");
        assert_eq!(*fetcher.0.borrow(), 0, "refused before fetching");
    });
    let log = String::from_utf8(captured.0.lock().expect("log").clone()).expect("utf-8");
    assert!(log.contains("category=\"duplicate_label\""), "{log}");
    assert!(log.contains("category=\"embedded_reference\""), "{log}");
    for secret in ["SECRET-LABEL", "SECRET-PATH", "\u{202e}"] {
        assert!(!log.contains(secret), "{secret} leaked: {log}");
    }
}
