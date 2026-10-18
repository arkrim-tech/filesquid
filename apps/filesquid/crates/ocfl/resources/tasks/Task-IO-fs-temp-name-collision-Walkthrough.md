# Task IO-fs-temp-name-collision: Walkthrough

## Problem

`FsBackend::unique_suffix` in [mod.rs](../../src/storage/fs/mod.rs) built temporary names and upload identifiers as `<pid>-<nanos>-<n>`, with `n` drawn from a per-instance counter. Two `FsBackend` instances in one process on one root both started at `n = 0`. When `SystemTime::now()` returned the same nanoseconds (coarse clocks, parallel threads), both chose the same `.fsquid-tmp-...` path. `create_new_checked` then failed with `Io { kind: AlreadyExists }`, so the losing writer of a create-only race saw an I/O error instead of `PreconditionFailed`. Observed on 2026-10-18 in `fs_create_only_has_one_winner_across_instances_with_separate_staging`:

```text
unexpected error Io { key: ".../root/objects/a/.fsquid-tmp-19321-1792338462960812581-0", kind: AlreadyExists }
```

Traceability: [OCFL-S-007 storage backend](../../tests/features/storage_backend_OCFL-S-007.feature).

## Change

- Replaced the per-instance `next_id` field with a process-wide `static NEXT_SUFFIX: AtomicU64`. Backend instances in one process never reuse a sequence number, whatever the clock resolution. Processes running at the same time have distinct process ids.
- The nanosecond timestamp lowers the chance that a reused process id meets stale temporary files left by an earlier, crashed process. Wall-clock time can move backwards, so it is not a uniqueness guarantee. If such a stale file does exist, `create_new_checked` refuses it with `AlreadyExists` rather than reusing it.
- The `.fsquid-tmp-` prefix and the suffix format are unchanged.
- Two unit tests in `storage::fs::tests`:
  - `fresh_instances_on_one_root_never_share_a_sequence_number` fails deterministically on the old code.
  - `concurrent_instances_on_one_root_never_repeat_a_suffix` draws 3,200 suffixes from 16 instances on 16 threads and asserts no duplicates.

## Validation

Failing test before the fix (`cargo test -p filesquid-ocfl --locked --lib storage::fs::tests`):

```text
test storage::fs::tests::concurrent_instances_on_one_root_never_repeat_a_suffix ... ok
test storage::fs::tests::fresh_instances_on_one_root_never_share_a_sequence_number ... FAILED
thread 'storage::fs::tests::fresh_instances_on_one_root_never_share_a_sequence_number' (2737) panicked at crates/ocfl/src/storage/fs/mod.rs:477:9:
assertion `left != right` failed: 2735-1792338946257085447-0 and 2735-1792338946257088745-0 share a sequence
  left: "0"
 right: "0"
test result: FAILED. 1 passed; 1 failed; 0 ignored; 0 measured; 33 filtered out; finished in 0.10s
```

After the fix, the same command:

```text
test storage::fs::tests::fresh_instances_on_one_root_never_share_a_sequence_number ... ok
test storage::fs::tests::concurrent_instances_on_one_root_never_repeat_a_suffix ... ok
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 33 filtered out; finished in 0.01s
```

`cargo fmt --all -- --check` passed, and `cargo clippy --workspace --all-targets --locked -- -D warnings` exited 0.

`cargo test -p filesquid-ocfl --locked` (run from `apps/filesquid/`):

```text
     Running unittests src/lib.rs (target/debug/deps/filesquid_ocfl-b2c12735eebebbfd)
test result: ok. 35 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.46s
     Running tests/features.rs (target/debug/deps/features-e8749b82317a3f52)
95 scenarios (91 passed, 4 skipped)
413 steps (409 passed, 4 skipped)
     Running tests/integration/fixture_conformance.rs (target/debug/deps/integration_fixture_conformance-d9830e6bb33768f2)
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.14s
     Running tests/integration/fixture_inventories.rs (target/debug/deps/integration_fixture_inventories-6780457da55c6a7f)
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
     Running tests/integration/fixture_reads.rs (target/debug/deps/integration_fixture_reads-65694b3784710fa8)
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.41s
     Running tests/integration/fs_conformance.rs (target/debug/deps/integration_fs_conformance-cfb3aa6b55ad7734)
test result: ok. 46 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.97s
     Running tests/integration/hierarchy_guard.rs (target/debug/deps/integration_hierarchy_guard-f112164d38bb3472)
test result: ok. 18 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.05s
     Running tests/integration/logical_file.rs (target/debug/deps/integration_logical_file-997ac9fb43951e16)
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.77s
     Running tests/integration/memory_conformance.rs (target/debug/deps/integration_memory_conformance-8b1e7214bf0eebdf)
test result: ok. 18 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.88s
     Running tests/integration/read_objects.rs (target/debug/deps/integration_read_objects-2f699af2cbb52937)
test result: ok. 31 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.17s
     Running tests/integration/validate_objects.rs (target/debug/deps/integration_validate_objects-e075ad7cfb7f3a50)
test result: ok. 40 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.45s
     Running tests/integration/write_order.rs (target/debug/deps/integration_write_order-dfa51f46977847eb)
test result: ok. 23 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.19s
     Running tests/unit/digest.rs (target/debug/deps/unit_digest-fa42e97933610306)
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s
     Running tests/unit/fixity.rs (target/debug/deps/unit_fixity-b525122df0e6f047)
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/unit/git_guard.rs (target/debug/deps/unit_git_guard-d90e2e7805cb87d9)
test result: ok. 20 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.12s
     Running tests/unit/inventory.rs (target/debug/deps/unit_inventory-509794a7dfc41144)
test result: ok. 35 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.05s
     Running tests/unit/lease.rs (target/debug/deps/unit_lease-05d3a8da3eeba326)
test result: ok. 17 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/unit/namaste_sidecar.rs (target/debug/deps/unit_namaste_sidecar-844a9744d9e8013a)
test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
     Running tests/unit/read_state.rs (target/debug/deps/unit_read_state-9939144c21cf88ec)
test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/unit/source_scan.rs (target/debug/deps/unit_source_scan-c0bcb612e76bbb27)
test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/unit/storage_key.rs (target/debug/deps/unit_storage_key-b107cca5cf8a330c)
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

The create-only race tests (`cargo test -p filesquid-ocfl --locked --test integration_fs_conformance fs_create_only`) passed in 20 of 20 repeated runs.

## Environment note

The BDD harness (`tests/features.rs`) builds the crate for `wasm32-unknown-unknown`. The first run failed in this environment because that target was missing. Running `rustup target add wasm32-unknown-unknown` fixed it. No values were mocked.
