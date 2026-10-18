# Task IO-0.6 Walkthrough: Local write order and unpublished vN

Module plan row: [IO-0.6](../plan.md). Global task: [Task 0.4](../../../../../../resources/implementation-plan.md#task-0.4), which stays open until IO-0.10. Branch `feat/io-0.6-write-order`, based on `main` at `9d5ec90` (IO-0.5 merged).

## OCFL-S-018: Write order and unpublished versions (reserve `vN` before any `vN` byte through an exclusive, fenced lease or reservation; stale holders write nothing; content, version inventory and sidecar, then root inventory and sidecar last; an incomplete `vN` is unpublished and reconciled under a new reservation)

Sources: [OCFL-S-018](../srs.md) (new row, see Sync), [PRD F-042](../../../../../../resources/prd.md#f-042-ocfl-preservation-and-portable-recovery), [ADR 007](../../../../../../resources/architecture/007-durable-jobs-and-interactive-performance.md) (exclusive leases), [ADR 012](../../../../../../resources/architecture/012-s3-compatible-ciphertext-storage.md) (conflict guard), [ADR 016](../../../../../../resources/architecture/016-rust-ocfl-library.md) ("Write (forward delta)" steps 2 to 7, "Published head (readers)").

## SRS-NFR-001: Preservation and transactional integrity

Sources: [PRD NFR-001](../../../../../../resources/prd.md#nfr-001-preservation-and-transactional-integrity), [SRS-NFR-001](../../../../../../resources/srs.md#srs-nfr-001-preservation-and-transactional-integrity). Crash fixtures at every publication step produce zero silent loss and zero false-complete versions.

## SRS-F-042: OCFL preservation and portable recovery

Sources: [SRS-F-042](../../../../../../resources/srs.md#srs-f-042-ocfl-preservation-and-portable-recovery). "Writes MUST serialize competing changes per object and recover interrupted content/inventory publication. OCFL writers MUST reserve `vN` before publishing object-root bytes, write the root inventory sidecar last, and treat incomplete `vN` as unpublished."

## OCFL-S-004: Only the root inventory commits a version; a `vN` above its `head` is unpublished

The reader now also treats a root inventory whose sidecar still commits the previous version's inventory as an unpublished head (ADR 016: "or the root sidecar is missing or mismatched").

## OCFL-S-016: Concurrent commits (every commit goes through the `vN` reservation and head compare-and-swap)

IO-0.6 builds the reservation and the head compare-and-swap. Rebase onto a moved head is the change model's job (IO-0.10); this layer reports `CommitError::HeadMoved` without writing.

## What changed

- **New public module `write`** (every target, `wasm32` included; no `std::fs`, no executor, no new dependency):
  - `VersionDraft` / `DraftFile`: the next version's whole logical state. Each path is `Staged(key)` (bytes on the caller's staging backend, outside the storage root) or `Existing(digest)` (bytes the manifest holds). Builder methods `stage`, `keep`, `set_message`, `set_user`.
  - `ObjectWriter::new(&storage, &reservations, holder).commit(root, expected_head, &staging, &draft) -> CommitReceipt`. Steps:
    1. Read the base (committed inventory and root inventory tag) and refuse a head other than `expected_head`.
    2. Assemble outside the object root: hash every staged file (sha256, 8 MiB chunks), build the new inventory (earlier versions kept, full state, manifest extended, `head` = `vN`), and check it with `Inventory::parse`. Only digests the manifest lacks get a content path, `vN/<contentDirectory>/<first logical path>`, once each; manifest digests keep the manifest's spelling (no E050/E096 from case). Invalid drafts (`created` not ending in `Z` or not RFC 3339, unknown digest, another object's id, forbidden path) are `CommitError::InvalidDraft`; a base that is not sha256 is `CommitError::UnsupportedAlgorithm`. Nothing is reserved or written for these.
    3. Reserve `vN` (`VersionReservations::reserve`).
    4. Under the reservation: re-read the base and re-check the head, run the Git guard and the hierarchy check on the object root, and reconcile `vN`: restore a root inventory whose sidecar was never written (compare-and-swap on its tag), keep each content file whose bytes are exactly the planned upload for that path, delete every other key under `vN/`.
    5. Write the declaration (new objects; an identical leftover is kept), each new bitstream (multipart, parts of at most 64 MiB, re-verified against the assembled digest, `IfAbsent`), the version inventory and its sidecar (`IfAbsent`), check that `vN/` lists exactly the planned keys with unchanged tags, then the root inventory (`IfMatch` the base tag, `IfAbsent` for `v1`) and the root sidecar last (`IfMatch` the sidecar read just before, `IfAbsent` for `v1`). A fence check (`VersionReservations::confirm`) precedes every part, every inventory write and both root writes.
    6. Release the reservation (not when it was lost) and return `CommitReceipt { object_id, version, created, inventory_digest, written }`. The caller records it; the engine owns no database (ADR 016 steps 1 and 7).
  - `VersionReservations` (reserve, confirm, release) and `Reservation` (object id, version, holder, fencing token). A host MAY implement the trait over a PostgreSQL generation (ADR 012).
  - `LeaseStore<B, C>` with `LeaseConfig { ttl_ms, margin_ms }` (defaults 60 s / 10 s): one JSON lease record per object at `<prefix>/<sha256 of object id>.json` in any compare-and-swap backend outside the storage root, written only with `put_if`. A takeover of an expired or released lease increments the token; records are never deleted, so tokens only grow. `confirm` fails (`ReservationError::Lost`) when the record no longer names this holder, token and version, or when no more than the margin is left; it renews through compare-and-swap once less than half the TTL is left. A margin of half the TTL or more is refused.
  - `Clock`, `SystemClock` (native only) and `rfc3339_utc(millis)`.
  - `CommitError` and `ReservationError`.
- **Reader** (`src/read/committed.rs`, moved from `object.rs`): when the root sidecar does not match the root inventory but matches the previous version's inventory, the object opens at that previous version and `Object::unpublished_head()` names the claimed head; any other mismatch is still E060. `read_small_tagged` returns the entity tag the head compare-and-swap needs.
- **Tests:** `tests/unit/lease.rs`, `tests/integration/write_order.rs` (every scenario on `MemoryBackend` and on a real directory through `FsBackend`), `tests/support/fault_backend.rs` (write log shared across backends, crash injection, post-write hooks), `tests/support/write_rig.rs`, and [`write_order_OCFL-S-018.feature`](../../tests/features/write_order_OCFL-S-018.feature) with steps in `tests/support/write_steps.rs`.
- **Docs:** crate [README](../../README.md) "Write" and "Read and browse"; module [SRS](../srs.md) OCFL-S-018; module [plan](../plan.md); [component diagram](../component-diagram.html); `lib.rs` crate docs; repository-root implementation-plan checklist.

## How crashes and stale holders are tested

- **Crash injection.** `FaultBackend::crash_at(key)` makes the next write of `key` return a future that never completes. The commit is driven with `complete_immediately`, so it is dropped mid-write exactly like a killed process: no cleanup runs and its lease stays held. Crash points: a content file (`complete obj/v2/content/e.txt`), the version inventory, the version sidecar (BDD), the root inventory, the root sidecar, and the first root sidecar of a new object. After each crash the object opens at `v1`, pinning `v2` is `VersionNotFound`, a second writer is refused (`Held`) until the lease lapses, and its retry publishes `v2` once (history `v1, v2`) with zero validation errors. A crash before the root sidecar opens at `v1` with `unpublished_head() == v2`; a crash before the first root sidecar leaves no committed version (E058) and the retry publishes `v1`.
- **Stale holders.** `FaultBackend::after_write(key, hook)` runs the hook right after `key` is written: it moves the clock past the TTL and lets `writer-b` reserve. Reassigned after its first content file, `writer-a` gets `Lost` and never writes the second file, the version inventory or the head. Reassigned after its version sidecar, it gets `Lost` and never writes `obj/inventory.json` or its sidecar (the write log shows no root write).
- **Write order.** One write log spans the storage backend and the lease backend: the reservation is the first write and the release the last; storage writes are exactly content, `v2/inventory.json`, `v2/inventory.json.sha256`, `inventory.json`, `inventory.json.sha256`; for `v1` the declaration is the first storage write after the reservation.

## TDD: failing run before implementation

The tests were written first; before `src/write/` existed they did not compile:

```text
error[E0432]: unresolved import `filesquid_ocfl::write`
 --> crates/ocfl/tests/unit/lease.rs:8:21
  |
8 | use filesquid_ocfl::write::{
--
error[E0432]: unresolved import `filesquid_ocfl::write`
  --> crates/ocfl/tests/integration/write_order.rs:16:21
   |
16 | use filesquid_ocfl::write::{CommitError, ReservationError, VersionDraft, VersionReservations};
--
error[E0432]: unresolved import `filesquid_ocfl::write`
  --> crates/ocfl/tests/support/write_steps.rs:10:21
   |
10 | use filesquid_ocfl::write::{
--
error: could not compile `filesquid-ocfl` (test "unit_lease") due to 1 previous error
error: could not compile `filesquid-ocfl` (test "integration_write_order") due to 2 previous errors
error: could not compile `filesquid-ocfl` (test "features") due to 2 previous errors
```

## Raw test output

`cargo test -p filesquid-ocfl --locked` from `apps/filesquid/` (executable paths removed):

```text
     Running tests/integration/write_order.rs

running 23 tests
test a_crash_before_the_first_root_sidecar_publishes_nothing ... ok
test a_crash_before_the_root_inventory_leaves_vn_unpublished ... ok
test a_crash_between_content_and_inventories_leaves_vn_unpublished ... ok
test a_crash_before_the_root_sidecar_reads_as_unpublished ... ok
test a_crash_while_writing_content_leaves_vn_unpublished ... ok
test a_git_directory_in_the_object_root_refuses_the_commit ... ok
test a_head_without_its_version_inventory_is_refused ... ok
test a_moved_head_is_refused_without_publishing ... ok
test a_reservation_held_by_another_writer_refuses_the_commit ... ok
test a_new_object_commits_as_v1_and_validates ... ok
test a_new_object_is_reserved_before_any_object_root_byte ... ok
test an_object_that_does_not_use_sha256_is_refused ... ok
test an_uncommitted_first_version_cannot_be_taken_over_by_another_object ... ok
test a_stale_lease_holder_cannot_publish_the_head_after_reassignment ... ok
test a_stale_lease_holder_cannot_write_content_after_reassignment ... ok
test creating_an_object_that_exists_is_refused ... ok
test an_unpublished_vn_is_reported_by_the_validator ... ok
test invalid_drafts_are_refused_before_any_reservation ... ok
test earlier_versions_and_inventories_are_never_changed ... ok
test staged_bytes_that_change_after_assembly_are_refused ... ok
test leftovers_of_an_unpublished_vn_are_reconciled_under_the_reservation ... ok
test the_next_version_writes_only_new_bytes ... ok
test writes_follow_the_publication_order ... ok

test result: ok. 23 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.22s
     Running tests/unit/lease.rs

running 17 tests
test a_lease_within_its_safety_margin_fails_its_fence_check ... ok
test a_corrupt_lease_record_is_reported ... ok
test a_clock_that_cannot_tell_the_time_fails_closed ... ok
test a_margin_of_half_the_ttl_or_more_is_refused ... ok
test a_record_too_large_to_read_back_is_not_written ... ok
test a_released_lease_is_lost_and_its_token_keeps_growing ... ok
test a_record_at_the_largest_token_is_not_wrapped ... ok
test a_held_lease_refuses_every_other_reservation ... ok
test an_expired_lease_fails_its_fence_check_even_before_takeover ... ok
test a_reservation_names_its_object_version_holder_and_first_token ... ok
test an_expired_lease_cannot_be_released ... ok
test objects_are_reserved_independently ... ok
test confirming_renews_a_lease_past_half_its_ttl ... ok
test lease_records_live_under_the_store_prefix ... ok
test rfc3339_utc_formats_unix_milliseconds ... ok
test an_expired_lease_is_taken_over_with_a_higher_token ... ok
test releasing_a_lost_reservation_is_refused_and_changes_nothing ... ok

test result: ok. 17 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
```

The BDD feature (part of `tests/features.rs`):

```text
Feature: Local write order and unpublished versions
  Scenario: A new version is published in write order
   ✔> Given an OCFL object "obj" committed at "v1" through the engine writer
   ✔  When writer "writer-a" commits "v2" adding "d.txt"
   ✔  Then the reservation should come before every "v2" byte
   ✔  And the writes should end with the version inventory, the root inventory and the root sidecar
   ✔  And the object head should be "v2"
   ✔  And the object should validate with zero errors
  Scenario: Unchanged and renamed files reuse their stored bytes
   ✔> Given an OCFL object "obj" committed at "v1" through the engine writer
   ✔  When writer "writer-a" commits "v2" renaming "b.txt" to "c.txt"
   ✔  Then no content should be written under "v2"
   ✔  And "c.txt" should resolve to the content path "v1/content/b.txt"
  Scenario Outline: A crash leaves the new version unpublished until it is recovered
   ✔> Given an OCFL object "obj" committed at "v1" through the engine writer
   ✔  Given writer "writer-a" crashed while writing "obj/v2/content/d.txt" during the commit of "v2"
   ✔  When I open the written object
   ✔  Then the object head should be "v1"
   ✔  And pinning "v2" should be refused as an unknown version
   ✔  When the crashed writer's lease lapses and writer "writer-b" retries the commit
   ✔  Then the object head should be "v2"
   ✔  And the history should list "v1, v2"
   ✔  And the object should validate with zero errors
  Scenario Outline: A crash leaves the new version unpublished until it is recovered
   ✔> Given an OCFL object "obj" committed at "v1" through the engine writer
   ✔  Given writer "writer-a" crashed while writing "obj/v2/inventory.json.sha256" during the commit of "v2"
   ✔  When I open the written object
   ✔  Then the object head should be "v1"
   ✔  And pinning "v2" should be refused as an unknown version
   ✔  When the crashed writer's lease lapses and writer "writer-b" retries the commit
   ✔  Then the object head should be "v2"
   ✔  And the history should list "v1, v2"
   ✔  And the object should validate with zero errors
  Scenario Outline: A crash leaves the new version unpublished until it is recovered
   ✔> Given an OCFL object "obj" committed at "v1" through the engine writer
   ✔  Given writer "writer-a" crashed while writing "obj/inventory.json" during the commit of "v2"
   ✔  When I open the written object
   ✔  Then the object head should be "v1"
   ✔  And pinning "v2" should be refused as an unknown version
   ✔  When the crashed writer's lease lapses and writer "writer-b" retries the commit
   ✔  Then the object head should be "v2"
   ✔  And the history should list "v1, v2"
   ✔  And the object should validate with zero errors
  Scenario Outline: A crash leaves the new version unpublished until it is recovered
   ✔> Given an OCFL object "obj" committed at "v1" through the engine writer
   ✔  Given writer "writer-a" crashed while writing "obj/inventory.json.sha256" during the commit of "v2"
   ✔  When I open the written object
   ✔  Then the object head should be "v1"
   ✔  And pinning "v2" should be refused as an unknown version
   ✔  When the crashed writer's lease lapses and writer "writer-b" retries the commit
   ✔  Then the object head should be "v2"
   ✔  And the history should list "v1, v2"
   ✔  And the object should validate with zero errors
  Scenario: A second writer cannot reserve a version that is held
   ✔> Given an OCFL object "obj" committed at "v1" through the engine writer
   ✔  Given writer "writer-b" holds the reservation for "v2"
   ✔  When writer "writer-a" commits "v2" adding "d.txt"
   ✔  Then the commit should be refused because the reservation is held
   ✔  And no byte should be written under "v2"
  Scenario: A stale lease holder cannot write content after reassignment
   ✔> Given an OCFL object "obj" committed at "v1" through the engine writer
   ✔  Given the lease passes to writer "writer-b" after "obj/v2/content/d.txt" is written
   ✔  When writer "writer-a" commits "v2" adding "d.txt" and "e.txt"
   ✔  Then the commit should be refused because the reservation was lost
   ✔  And "obj/v2/content/e.txt" should not be stored
   ✔  And the object head should be "v1"
  Scenario: A stale lease holder cannot publish the head after reassignment
   ✔> Given an OCFL object "obj" committed at "v1" through the engine writer
   ✔  Given the lease passes to writer "writer-b" after "obj/v2/inventory.json.sha256" is written
   ✔  When writer "writer-a" commits "v2" adding "d.txt"
   ✔  Then the commit should be refused because the reservation was lost
   ✔  And the root inventory should be unchanged
   ✔  And the object head should be "v1"
  Scenario: A moved head is refused without publishing
   ✔> Given an OCFL object "obj" committed at "v1" through the engine writer
   ✔  Given writer "writer-b" has committed "v2" adding "d.txt"
   ✔  When writer "writer-a" commits on top of "v1" adding "e.txt"
   ✔  Then the commit should be refused because the head moved to "v2"
   ✔  And the object head should be "v2"
```

The whole crate (each test binary's result line):

```text
     Running unittests src/lib.rs
test result: ok. 33 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.36s
     Running tests/features.rs
95 scenarios (91 passed, 4 skipped)
413 steps (409 passed, 4 skipped)
     Running tests/integration/fixture_conformance.rs
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.19s
     Running tests/integration/fixture_inventories.rs
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
     Running tests/integration/fixture_reads.rs
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.52s
     Running tests/integration/fs_conformance.rs
test result: ok. 46 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.09s
     Running tests/integration/hierarchy_guard.rs
test result: ok. 18 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.06s
     Running tests/integration/logical_file.rs
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.47s
     Running tests/integration/memory_conformance.rs
test result: ok. 18 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.95s
     Running tests/integration/read_objects.rs
test result: ok. 31 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.17s
     Running tests/integration/validate_objects.rs
test result: ok. 40 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.43s
     Running tests/integration/write_order.rs
test result: ok. 23 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.22s
     Running tests/unit/digest.rs
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
     Running tests/unit/fixity.rs
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/unit/git_guard.rs
test result: ok. 20 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.11s
     Running tests/unit/inventory.rs
test result: ok. 35 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.06s
     Running tests/unit/lease.rs
test result: ok. 17 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
     Running tests/unit/namaste_sidecar.rs
test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/unit/read_state.rs
test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/unit/source_scan.rs
test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/unit/storage_key.rs
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

The 4 skipped scenarios are unchanged from `main`: they belong to IO-0.7 (content-fixture writes), the server crate and the UI package.

`cargo fmt --all`, `cargo clippy --workspace --all-targets --locked -- -D warnings`, `cargo build -p filesquid-ocfl --locked --no-default-features --target wasm32-unknown-unknown` and `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps` all exit 0.

## SRS traceability

| Requirement | Tests |
|---|---|
| OCFL-S-018 reserve `vN` before any `vN` or object-root byte | `integration_write_order`: `writes_follow_the_publication_order`, `a_new_object_is_reserved_before_any_object_root_byte`, `invalid_drafts_are_refused_before_any_reservation`; BDD "A new version is published in write order" |
| OCFL-S-018 exclusive, fenced lease; second writer refused | `unit_lease`: `a_held_lease_refuses_every_other_reservation`, `an_expired_lease_is_taken_over_with_a_higher_token`, `a_released_lease_is_lost_and_its_token_keeps_growing`, `objects_are_reserved_independently`; `integration_write_order`: `a_reservation_held_by_another_writer_refuses_the_commit`; BDD "A second writer cannot reserve a version that is held" |
| OCFL-S-018 expired or reassigned holder writes no content and publishes no head | `unit_lease`: `an_expired_lease_fails_its_fence_check_even_before_takeover`, `a_lease_within_its_safety_margin_fails_its_fence_check`, `releasing_a_lost_reservation_is_refused_and_changes_nothing`; `integration_write_order`: `a_stale_lease_holder_cannot_write_content_after_reassignment`, `a_stale_lease_holder_cannot_publish_the_head_after_reassignment`; BDD "A stale lease holder cannot write content after reassignment", "... cannot publish the head after reassignment" |
| OCFL-S-018 reservation checked before content and at head publication; head CAS | `integration_write_order`: the two stale-holder tests, `a_moved_head_is_refused_without_publishing`, `creating_an_object_that_exists_is_refused`; `unit_lease`: `confirming_renews_a_lease_past_half_its_ttl` |
| OCFL-S-018 earlier versions never changed | `integration_write_order`: `earlier_versions_and_inventories_are_never_changed` |
| OCFL-S-018 only new bitstreams under `vN/` | `integration_write_order`: `the_next_version_writes_only_new_bytes`; BDD "Unchanged and renamed files reuse their stored bytes" |
| OCFL-S-018 assemble outside the object root | `integration_write_order`: `staged_bytes_that_change_after_assembly_are_refused`, `invalid_drafts_are_refused_before_any_reservation` |
| OCFL-S-018 content, version inventory + sidecar, root inventory + sidecar last | `integration_write_order`: `writes_follow_the_publication_order`; BDD "A new version is published in write order" |
| OCFL-S-018 / OCFL-S-004 incomplete `vN` unpublished for readers | `integration_write_order`: the five crash tests, `an_unpublished_vn_is_reported_by_the_validator`; BDD scenario outline (4 crash points) |
| OCFL-S-018 recovery under a new reservation, no second `vN` | `integration_write_order`: the five crash tests (history `v1, v2`), `leftovers_of_an_unpublished_vn_are_reconciled_under_the_reservation` |
| SRS-NFR-001 zero silent loss, zero false-complete versions | crash tests: no crash point publishes `v2`, every retry validates with zero errors |
| SRS-F-042 per-object serialization and interrupted-publication recovery | as above |
| OCFL-S-001 guards before publication | `integration_write_order`: `a_git_directory_in_the_object_root_refuses_the_commit` |
| OCFL-S-002 sha256 only | `integration_write_order`: `a_new_object_commits_as_v1_and_validates`, `an_object_that_does_not_use_sha256_is_refused` |
| OCFL-S-007 no direct I/O; `wasm32` build | `unit_source_scan` (unchanged, covers `src/write/`); BDD "Engine builds for the browser" |
| RFC 3339 UTC `created` | `unit_lease`: `rfc3339_utc_formats_unix_milliseconds`; `integration_write_order`: `invalid_drafts_are_refused_before_any_reservation` |
| OCFL-S-009 written objects validate | every commit test ends with `assert_valid` (zero errors) |

## Coverage

[ADR 017](../../../../../../resources/architecture/017-test-runners-and-reporting.md) defines no Rust line-coverage tool, so no percentage is reported. These `CommitError` variants are produced by a test: `Reservation` (`Held`, `Lost`), `HeadMoved`, `InvalidDraft`, `UnsupportedAlgorithm`, `StagingChanged`, `MissingVersionInventory` and `Guard`. `Read` and `Storage` (backend failures) are not; `ForeignWrite` is reached only through the residual race described under "Remaining limits". `unit_lease` produces every `ReservationError` variant except `Storage` (backend failures): `Held`, `Lost`, `Corrupt`, `InvalidConfig` and `ClockUnavailable`. Crash tests cover these publication keys: a content file, the version inventory, the version sidecar (BDD), the root inventory, the root sidecar and the first root sidecar. The object declaration and individual multipart parts are not crash-injected.

## Review round 1 (Cubic)

Cubic reviewed `fcd83b6` and raised 22 findings. Fixed in code or docs:

- `base.rs`: an uncommitted first version keeps its object id (`Base::pending_id`); a draft for another id is `InvalidDraft` (test `an_uncommitted_first_version_cannot_be_taken_over_by_another_object`).
- `committed.rs`: the writer refuses a base whose committed head has no version inventory (`CommitError::MissingVersionInventory`, test `a_head_without_its_version_inventory_is_refused`), since readers need it to read past an interrupted publication.
- `publish.rs`: the root sidecar is now a compare-and-swap on the sidecar read before the root inventory is replaced, so a writer that lost its lease after its last fence check cannot overwrite a sidecar another writer published.
- `reconcile.rs`: on backends whose tags are content digests, a kept leftover must also hash to its tag (no change-and-change-back mix).
- `reconcile.rs` and `publish.rs`: one listing helper (`write/listing.rs`) bounds the walk by keys (`MAX_WALK_ENTRIES`) and checks every continuation with `guard::next_start`.
- `lease.rs`: the fencing token uses checked addition (`Corrupt` at `u64::MAX`), records above 4 KiB are refused before they are written, and an expired lease cannot be released (`Lost`). `Clock::now_millis` returns `Option<u64>`; a clock that cannot tell the time fails closed (`ReservationError::ClockUnavailable`). Tests for each in `unit_lease`.
- `error.rs`: an inventory above the engine's parsing bound is `CommitError::Read` (format error), not `InvalidDraft`.
- `plan.rs`: a content path no storage key can name (`\`, NUL) is `InvalidDraft` before anything is reserved (added to `invalid_drafts_are_refused_before_any_reservation`).
- Tests: crash scenarios check the leftovers before recovery; the fault backend forgets finished uploads.
- Docs: the README says `vN/<contentDirectory>/` and describes the first-version crash (E058); the component diagram shows the publisher as built in both SVGs and their accessible descriptions, and the caption no longer claims the change model routes writes yet; this Walkthrough's coverage claims are narrowed; the module SRS row OCFL-S-018 now states what the writer guarantees, including the open check-to-write window.

Answered rather than fixed: the stale-writer findings on the residual check-to-write window (SRS wording, a stalled recovering writer's delete). Storage-side token enforcement is not possible through the current `StorageBackend` trait and is an owner decision (see "Remaining limits").

## Review round 2 (Cubic)

Cubic reviewed `febe840` and raised 2 documentation findings, both fixed. The README now says which validation error each crash point leaves (E046 before the root inventory is replaced, E060 after it; E058 or E063 for a first commit). The coverage section above now lists `ClockUnavailable`.

## Sync Execution Report

- **Module SRS:** new row **OCFL-S-018** (write order and unpublished versions). It restates PRD F-042 ("reserve `vN` before publishing object-root bytes, write the root inventory sidecar last, and treat incomplete `vN` as unpublished"), ADR 007 (an expired worker MUST NOT commit), ADR 012 (fenced reservation; head CAS alone is not sufficient) and ADR 016 write steps 2 to 6, which the module SRS did not yet hold. No new requirement beyond those sources.
- **Module plan:** IO-0.6 checked; Related Requirements add OCFL-S-018, OCFL-S-004, OCFL-S-016 and ADR 012; Related Tests list the new tests and this Walkthrough.
- **Repository-root implementation-plan checklist:** IO-0.6 checked. Global Task 0.4 stays open (IO-0.7 to IO-0.10).
- **Crate README:** new "Write" section; "Read and browse" describes the unpublished-head fallback.
- **Component diagram:** the version publisher row is BUILT; the caption lists it.
- **PRD, global SRS, product vision, ADRs, root README:** unchanged. F-042 and NFR-001 already require this behaviour; no new command, variable or dependency.
- **BDD:** `write_order_OCFL-S-018.feature` (new). The server crate's `preservation_and_transactional_integrity_NFR-001.feature` is a placeholder owned by the platform engineer and is not edited here.

## Mocked Environment Variables

None. The tests use a hand-moved clock and in-memory or scratch-directory backends; no credential or `.env` value was mocked.

## Remaining limits

- **Fencing is checked, not storage-enforced (owner decision).** A writer confirms its lease before every write and stops once no more than the margin (default 10 s) is left. A writer that stalls longer than the margin between a fence check and its next write can still make that one write after its lease passed on. It cannot overwrite another writer's bytes (`IfAbsent` under `vN/`), root inventory or root sidecar (both compare-and-swaps). But an extra file it adds under `vN/` after the new holder's final listing check would be published and reported by the validator (E023), and a stalled recovering writer could delete a leftover the new holder had already verified. Closing this fully needs storage-side token checks (for example a conditional write keyed on the token in the S3 backend, IO-5.1, or a host PostgreSQL generation checked in the same transaction as the receipt).
- **One commit, one object.** The writer publishes one version of one object; FileSquid collection snapshots stay callers (ADR 023).
- **Rebase** onto a moved head is IO-0.10; this layer reports `HeadMoved`.
- **Base objects must use sha256.** Extending an object another tool wrote with sha512 is refused (`UnsupportedAlgorithm`), since the engine writes sha256 only (OCFL-S-002) and one inventory has one content-addressing algorithm.
- Content-fixture write conformance and community-validator checks of written objects are IO-0.7 (OCFL-S-010 (b)).
