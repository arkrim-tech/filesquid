//! In-memory backend conformance (OCFL-S-007): the shared suite run against
//! `MemoryBackend`. This target needs no Cargo features, so backend coverage
//! remains with `--no-default-features`.

use filesquid_ocfl::storage::MemoryBackend;

#[path = "../support/conformance.rs"]
mod suite;

/// An in-memory backend fixture.
struct MemoryFixture(MemoryBackend);

mod memory {
    use super::*;
    conformance_tests!(MemoryFixture(MemoryBackend::new()));
}
