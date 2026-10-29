# Task IO-1.7 Walkthrough: Streamed single-file stage

Branch `feat/io-1.7-streamed-stage-file`. Module task [IO-1.7](../plan.md) of global Task 1.2 ([implementation plan](../../../../../../resources/implementation-plan.md)). The desktop switch from `stage_bytes` to `stage_file` and the removal of its 512 MiB refusal are the desktop follow-up DE-1.9, not part of this task.

## OCFL-S-026: Streamed single-file stage (one logical file from any backend in bounded chunks, digest answered, manifest bytes not copied, a held blob re-hashed and reused or replaced, the source read at most twice and refused as SourceChanged when the reads differ, the OCFL-S-018 write order kept)

`ChangeModel::stage_file<D: StorageBackend>(&self, path: &str, source: &D, key: &str) -> Result<String, ChangeError>` (`src/change/model.rs`):

| Constraint | Implementation | Test |
|---|---|---|
| Bounded chunks of at most `MAX_CHUNK_BYTES` (8 MiB), never read whole | the staging area's `put_file` (the path behind `stage_tree`): `hash_file` and `copy_file` read through `ChunkReader` with `MAX_CHUNK_BYTES` | `a_file_larger_than_one_chunk_is_read_in_bounded_chunks`: a 16 MiB + 123 B file, largest read ≤ 8 MiB, exactly 2 × 3 chunk reads |
| Hashed as it is copied; the sha256 digest answered | `upload_parts` hashes each chunk as it uploads; `stage_file` returns the digest | `stage_file_answers_the_digest_and_stages_the_blob_stage_bytes_would` (same blob and same index as `stage_bytes`) |
| Bounded buffering: no whole-file buffer in the stage | chunk reads of at most `MAX_CHUNK_BYTES` and multipart parts of at most `MAX_PART_BYTES`, one of each in memory at a time; only the upload's per-part completion records (one small entry per part, at most `MAX_PARTS`) grow with the file. The staged blob itself lives in the staging backend: on disk or remote for `FsBackend` and remote stores, in memory for `MemoryBackend` (which keeps every part until the upload completes) | the bounded-read test above |
| Manifest bytes not copied (OCFL-S-014) | `known` (the manifest digests) checked after the hashing read | `a_file_whose_digest_the_manifest_holds_is_not_copied`: no blob write, one read pass, no new content on commit |
| A held blob re-hashed; reused when it matches | `holds_blob` re-hashes the blob | `a_blob_the_staging_area_holds_is_not_copied_again`: no blob write, one read pass |
| A corrupt or stale blob replaced before the index points at it | `holds_blob` reports a mismatch as absent and the copy replaces it; the index is saved after | `a_corrupt_or_stale_staged_blob_is_replaced_with_the_source_bytes`: one blob write, the blob and the committed content hold the source bytes |
| Read at most twice; `SourceChanged` and the partial copy removed | `put_file` compares the copy's digest with the hash pass and deletes the blob | `a_file_that_changes_between_the_two_reads_is_refused`: the source changes after the hash pass; no blob under either digest, no index |
| Path validated as `stage_bytes` does, before the file is read | `valid_path`, then `check_new_path` (E095 path conflicts) before `put_file` | `an_invalid_logical_path_is_refused_before_the_source_is_read` and `a_path_under_a_logical_file_is_refused_before_the_source_is_read` (`a.txt/inner` under the committed `a.txt`): no read, no staging write |
| The commit keeps the OCFL-S-018 write order | the ordinary commit path | `the_commit_of_a_streamed_file_keeps_the_write_order` on `MemoryBackend` and `FsBackend` storage |

The chunked-read, manifest, held-blob, corrupt-blob and changed-source integration scenarios run through `on_both_sources!`, reading their source from `MemoryBackend` and from `FsBackend`, each wrapped in the test-only `CountingBackend` (`tests/support/counting_backend.rs`), which counts and sizes ranged reads and can replace a file after a chosen number of reads. The two path checks read nothing, and the write-order test reads its source from memory and varies only the storage backend (`MemoryBackend` and `FsBackend`). In the BDD feature, only the bounded-chunk outline runs on both source backends; its other scenarios use the memory source.

## OCFL-S-014: Forward-delta change model (only bytes the manifest lacks are written)

Covered by the manifest test above; `receipt.written` is empty for a streamed copy of committed bytes.

## OCFL-S-015: Working state and staging (the staged file joins the working state like one staged from bytes)

The stage-bytes comparison test shows the same staging index and blob for `stage_file` and `stage_bytes`.

## OCFL-S-018: Write order and unpublished versions

`the_commit_of_a_streamed_file_keeps_the_write_order`: the lease reservation first, then `complete obj/v2/content/big.raw`, the version inventory and its sidecar, the root inventory and its sidecar, and the release last.

## SRS-NFR-003: Bounded processing and migration endurance

The stage's own buffering is bounded whatever the file's size: chunk reads of at most 8 MiB and multipart parts of at most 64 MiB, one of each at a time, and only the upload's per-part completion records (one small entry per part of up to 64 MiB, at most 10,000 parts) grow with the file. The staging backend then holds the staged blob as its storage: on `FsBackend` or a remote store it is on disk or remote, while `MemoryBackend` is in-memory storage and keeps every uploaded part until the upload completes, so a staging area on `MemoryBackend` holds the whole staged file in memory by design. A host stages large files on `FsBackend` or a remote store.

## BDD

`streamed_stage_OCFL-S-026.feature` (steps in `tests/support/stage_file_steps.rs`): the bounded-chunk outline on the memory and filesystem backends with a commit that keeps the OCFL-S-018 write order, manifest bytes not copied (read once, no blob in the staging area), a corrupt or stale staged blob replaced with the source's bytes, an invalid logical path refused before the source is read, and a source that changes between its reads.

## What changed

- `src/change/model.rs`: `ChangeModel::stage_file` and the private `check_new_path`.
- Tests: `tests/integration/stage_file.rs` (new `[[test]]` `integration_stage_file`), `tests/support/counting_backend.rs`, `tests/features/streamed_stage_OCFL-S-026.feature`, `tests/support/stage_file_steps.rs`; `change_steps::rig` is now visible to other step files.
- Docs: crate README "Change model" (the `stage_file` bullet and its two test commands, both run before documenting), plan row and checklist.

## TDD: failing run before the implementation

```text
error[E0599]: no method named `stage_file` found for struct `ChangeModel<'a, B, R, S>` in the current scope
   --> crates/ocfl/tests/integration/stage_file.rs:136:47
    |
136 |     let digest = ready(model(&rig, &streamed).stage_file("photo.raw", source, SOURCE_KEY))
    |                                               ^^^^^^^^^^
    |
help: there is a method `stage_tree` with a similar name, but with different arguments
--
error[E0599]: no method named `stage_file` found for struct `ChangeModel<'a, B, R, S>` in the current scope
   --> crates/ocfl/tests/integration/stage_file.rs:159:26
    |
159 |     let digest = ready(m.stage_file("big.raw", source, SOURCE_KEY)).expect("stage_file");
    |                          ^^^^^^^^^^
    |
help: there is a method `stage_tree` with a similar name, but with different arguments
--
error[E0599]: no method named `stage_file` found for struct `ChangeModel<'a, B, R, S>` in the current scope
   --> crates/ocfl/tests/integration/stage_file.rs:184:26
    |
184 |     let digest = ready(m.stage_file("copy-of-a.txt", source, SOURCE_KEY)).expect("stage_file");
    |                          ^^^^^^^^^^
    |
help: there is a method `stage_tree` with a similar name, but with different arguments
--
error[E0599]: no method named `stage_file` found for struct `ChangeModel<'a, B, R, S>` in the current scope
   --> crates/ocfl/tests/integration/stage_file.rs:206:28
    |
206 |     let answered = ready(m.stage_file("b.txt", source, SOURCE_KEY)).expect("stage_file");
    |                            ^^^^^^^^^^
    |
```

## Mocked Environment Variables

None.

## Raw test output

`cargo test -p filesquid-ocfl --locked --test integration_stage_file`:

```text
    Finished `test` profile [unoptimized] target(s) in 2.00s
     Running tests/integration/stage_file.rs (<target>/debug/deps/integration_stage_file-281404c847806322)

running 9 tests
test a_blob_the_staging_area_holds_is_not_copied_again ... ok
test a_corrupt_or_stale_staged_blob_is_replaced_with_the_source_bytes ... ok
test a_path_under_a_logical_file_is_refused_before_the_source_is_read ... ok
test an_invalid_logical_path_is_refused_before_the_source_is_read ... ok
test stage_file_answers_the_digest_and_stages_the_blob_stage_bytes_would ... ok
test a_file_whose_digest_the_manifest_holds_is_not_copied ... ok
test a_file_that_changes_between_the_two_reads_is_refused ... ok
test the_commit_of_a_streamed_file_keeps_the_write_order ... ok
test a_file_larger_than_one_chunk_is_read_in_bounded_chunks ... ok

test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 19.51s
```

`cargo test -p filesquid-ocfl --locked --test features -- --input "streamed_stage_*.feature"`:

```text
    Finished `test` profile [unoptimized] target(s) in 19.39s
     Running tests/features.rs (<target>/debug/deps/features-9009fa7eceb51bdd)
Feature: Streamed single-file stage
  Scenario Outline: A file larger than one chunk is staged in bounded chunks from the memory backend
   ✔> Given an object committed at "v1" with "a.txt", "b.txt" and "c.txt" through the change model
   ✔  Given a source file of three chunks on the memory backend
   ✔  When writer "writer-a" stages it as "big.raw" with stage_file
   ✔  Then stage_file should answer the sha256 digest of the source file
   ✔  And the source should have been read in chunks of at most 8 MiB, once to hash it and once to copy it
   ✔  When writer "writer-a" commits the streamed file
   ✔  Then the committed "big.raw" should hold the source file's bytes
   ✔  And the commit should keep the OCFL-S-018 write order for "big.raw"
  Scenario Outline: A file larger than one chunk is staged in bounded chunks from the filesystem backend
   ✔> Given an object committed at "v1" with "a.txt", "b.txt" and "c.txt" through the change model
   ✔  Given a source file of three chunks on the filesystem backend
   ✔  When writer "writer-a" stages it as "big.raw" with stage_file
   ✔  Then stage_file should answer the sha256 digest of the source file
   ✔  And the source should have been read in chunks of at most 8 MiB, once to hash it and once to copy it
   ✔  When writer "writer-a" commits the streamed file
   ✔  Then the committed "big.raw" should hold the source file's bytes
   ✔  And the commit should keep the OCFL-S-018 write order for "big.raw"
  Scenario: Bytes the object already holds are not copied
   ✔> Given an object committed at "v1" with "a.txt", "b.txt" and "c.txt" through the change model
   ✔  Given a source file holding the bytes of "a.txt" on the memory backend
   ✔  When writer "writer-a" stages it as "copy-of-a.txt" with stage_file
   ✔  Then stage_file should answer the sha256 digest of the source file
   ✔  And the source should have been read once, to hash it
   ✔  And no blob should have been copied into the staging area
  Scenario: A corrupt or stale staged blob is replaced with the source's bytes
   ✔> Given an object committed at "v1" with "a.txt", "b.txt" and "c.txt" through the change model
   ✔  Given a source file of three chunks on the memory backend
   ✔  And the staging area already holds other bytes under the source file's digest
   ✔  When writer "writer-a" stages it as "big.raw" with stage_file
   ✔  Then stage_file should answer the sha256 digest of the source file
   ✔  And the staged blob should hold the source file's bytes
  Scenario: An invalid logical path is refused before the source is read
   ✔> Given an object committed at "v1" with "a.txt", "b.txt" and "c.txt" through the change model
   ✔  Given a source file of three chunks on the memory backend
   ✔  When writer "writer-a" stages it as "../escape" with stage_file
   ✔  Then stage_file should be refused as an invalid logical path
   ✔  And the source should not have been read
  Scenario: A source that changes between its two reads is refused
   ✔> Given an object committed at "v1" with "a.txt", "b.txt" and "c.txt" through the change model
   ✔  Given a source file of three chunks on the memory backend that changes after it is hashed
   ✔  When writer "writer-a" stages it as "big.raw" with stage_file
   ✔  Then stage_file should be refused because the source changed
   ✔  And no staged blob should be left under either digest
[Summary]
1 feature
6 scenarios (6 passed)
38 steps (38 passed)
```
