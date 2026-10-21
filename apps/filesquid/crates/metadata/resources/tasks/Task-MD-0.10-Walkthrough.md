# Task MD-0.10 Walkthrough: Cited-version check for import commits and the `OriginalName` scrub class

Branch: `feat/md-0.10-cited-version-original-name-scrub` (completes global Task FS-1.3). Owner decision 2026-10-20, items 2(b) "check" and 2(e) "add class" on the open points of the [MD-0.8 walkthrough](./Task-MD-0.8-Walkthrough.md); [ADR 038](../../../../../../resources/architecture/038-import-file-record.md) Decisions 6 and 10. Item 2(a) "keep" needs no code.

## MD-S-008: Canonical metadata is `metadata.md` (the approved exception for import commits)

Inside the commit, the store checks that the `object_version` each new import entry cites is the version being committed. An import commit whose base head has moved is not rebased: it fails as a retryable conflict that publishes nothing. Statement edits still rebase.

## MD-S-013: Import record (the `OriginalName` identity class)

`OriginalName` is the original file name wherever the record holds it: the header's original name, the display name and each import entry's source file name. It is scrubbed by default from shares and publications, kept only when the saved outgoing policy selects **Use original filename**, and the store refuses to save a record whose original name is withheld.

## What changed

- `store::ocfl::cited::CitedContent`, the version-bound-files check that `commit_import_with` and `append_import_with` already ran inside the commit (after the fenced `vN` reservation and again after each rebase), now also checks that every new import entry's cited `object_version` is the version being committed. The content check runs first, then the version check. When either fails, the commit publishes nothing and returns `StoreError::Conflict` with the reason ("the import entry cites v2, but the commit would publish v3; retry citing the new version"). A rebase always changes the reserved version, so an import commit that would rebase is refused; the staged content of `commit_import_with` stays, so the caller retries citing the new version. Statement edits (`commit_record_with`, `save`) do not use the check and still rebase.
- `import::scrub::IdentityClass::OriginalName`, in `IdentityClass::ALL` (now 8 classes, the default for shares and publications). `ImportEntry::scrubbed` withholds `source.original_name`. The module documentation no longer says the original name is not a class.
- `import::SourceFile::original_name` stays a `Raw`, so the struct literals and field reads of DE-1.2 (merged as PR #115 while this task ran: `crates/native/src/import/facts.rs`, `crates/api/src/files/import.rs`) compile unchanged. A withheld source name is the empty name, which no source reports: a serde adapter writes it as `{"unavailable":"withheld_by_policy"}` and reads that fact back as the empty name, and reading refuses an empty name string and any other unavailable reason. `SourceFile::name_withheld()` tells the two apart. A known name serialises exactly as before (a string, or the base64 map), so every stored record reads unchanged.
- `file_record::FileRecord::scrubbed(&[IdentityClass])`: the outgoing copy of the whole record. Every import entry is scrubbed of the classes; with `OriginalName` the header's `original_name` and `display_name` are written as `{"unavailable":"withheld_by_policy"}`. Extension, format, media type and statements stay. The copy renders and reads back (`from_markdown` accepts both names withheld by policy; one name withheld, or a name unavailable for another reason, is `Malformed`). `names_withheld()` reports a header or any entry source name withheld; `set_display_name` on such a copy is `RecordError::NamesWithheld`.
- The `ocfl` store refuses to save a record whose names are withheld (`StoreError::Record(RecordError::NamesWithheld)`, nothing published) in `commit_file_record_with`, `commit_import_with`, `append_import_with` and a statements-only save over such a record.
- Signatures of `commit_import_with`, `append_import_with`, `commit_file_record_with` and every `FileRecord` accessor are unchanged.

## Cubic round 1 changes

- P2: a record could hold a scrubbed entry beside a header with its names. A record is now scrubbed of `OriginalName` whole or not at all. `FileRecord::append_import` refuses an entry whose source name is withheld when the record's names are not, and the other way round (`RecordError::NamesWithheld`). `validate` (so `from_markdown` too) refuses a record withheld in only part (`Malformed`). The header follows `names_withheld()`. Test: `the_original_name_is_withheld_in_the_whole_record_or_nowhere`.
- P3: the two requirement headings now carry their requirement text.
- P3 (empty source name in `ImportEntry::validate`): not changed, answered on the thread. A scrubbed entry has the empty name and must stay valid. A known name is never empty: reading refuses an empty name, and the store refuses to save a withheld one.

## Interface notes for other lanes

- **DE-1.2 (PR #115, now on `main`) needs no source change**: every public signature and field type it uses is unchanged. Its import entries cite the version they are committed as, so the new check passes for a commit that does not race; a race is now a retryable `StoreError::Conflict` instead of a rebased record.
- Every import caller now gets a retryable `StoreError::Conflict` when another writer commits to the object between staging and publishing; it retries with the new head's next version.
- The sharing routes (API-F1.13 follow-up) apply `FileRecord::scrubbed(&IdentityClass::ALL)` to a shared or published record, leaving `OriginalName` out when the saved outgoing policy selects **Use original filename**.

## SRS traceability

| Requirement | Tests |
|---|---|
| MD-S-008 Canonical metadata is `metadata.md`: inside the commit, the cited `object_version` equals the version being committed | `an_import_commit_whose_cited_version_matches_publishes_it`, `an_import_commit_citing_another_version_is_a_conflict`, `a_re_import_citing_an_earlier_version_is_a_conflict`; BDD *An import commit checks the version its entry cites* |
| MD-S-008 Canonical metadata is `metadata.md`: an import commit that would rebase is a retryable conflict that publishes nothing | `an_import_commit_that_would_rebase_is_a_retryable_conflict_that_publishes_nothing`, `a_re_import_that_would_rebase_is_a_retryable_conflict_that_publishes_nothing` (each retries citing `v3` and publishes it); BDD *An import commit checks the version its entry cites* |
| MD-S-008 Canonical metadata is `metadata.md`: statement edits still rebase | `a_statements_edit_still_rebases_over_a_moved_head`; BDD *A statements edit still rebases over another writer's version* |
| MD-S-013 Import record: `OriginalName` is an identity class, scrubbed by default | `every_class_includes_the_original_name`, `an_entry_scrub_withholds_the_source_name` |
| MD-S-013 Import record: the header's original name and display name and every entry's source name are withheld as `withheld_by_policy` | `a_record_scrub_withholds_the_header_names_and_every_source_name`, `a_renamed_display_name_is_withheld_too`, `a_record_scrub_applies_the_other_classes_to_every_entry`, `a_scrubbed_record_reads_back_as_scrubbed`, `a_record_with_only_one_header_name_withheld_is_malformed`, `a_header_name_unavailable_for_another_reason_is_malformed`, `a_source_name_is_known_and_never_empty_or_unavailable_for_another_reason`, `the_original_name_is_withheld_in_the_whole_record_or_nowhere`; BDD *The original file name is withheld from an outgoing copy by default* |
| MD-S-013 Import record: the store refuses to save a scrubbed record | `the_store_refuses_to_save_a_scrubbed_record`, `the_store_refuses_to_import_a_scrubbed_record`, `the_store_refuses_to_append_an_entry_whose_source_name_is_withheld`, `a_scrubbed_record_cannot_be_renamed`; BDD *The original file name is withheld from an outgoing copy by default* |
| MD-S-013 Import record: **Use original filename** keeps the names | `a_scrub_without_the_original_name_keeps_every_name`; BDD *Use original filename keeps the names in the outgoing copy* |
| MD-S-013 Import record: no filesystem reads, `wasm32` build | existing BDD *The import record builds for the browser without filesystem access* and the `wasm32` build checks, still passing |

Existing tests updated: the second import entry in `tests/unit/file_record_store.rs` and `tests/features.rs` now cites `v2`, the version it is committed as.

## Mocked Environment Variables

None.

## Test output

All outputs below are from real runs; absolute paths are replaced with `<repo>`, the shared Cargo target with `<target>` and the scratch directory with `<tmp>`; `Compiling` lines are left out. Summary of the green run: lib 3, cli 31, csv 10, extract 13, file_record_store 34, file_ref 3, import_record 71 (70 in the pasted run below, which predates the Cubic round 1 test), isolation 4, json 19, markdown 3, namespace 47, namespace_store 6, register 10, sidecar 8, store 28 passed, 0 failed. BDD: 59 scenarios (58 passed, 1 skipped), 181 steps (180 passed, 1 skipped); the skipped step is the existing UI-only "Metadata panel" scenario, skipped before this task too. `cargo clippy -p filesquid-metadata --all-targets --all-features --locked -- -D warnings`, `cargo clippy -p filesquid-api -p filesquid-native --all-targets --locked -- -D warnings` and `RUSTDOCFLAGS="-D warnings" cargo doc -p filesquid-metadata --no-deps --all-features --locked` exit 0.

### Red: the cited-version tests before the check (scrub already implemented)

`cargo test -p filesquid-metadata --features ocfl --test unit_file_record_store --locked cited_version`

```text
Finished `test` profile [unoptimized] target(s) in 0.15s
     Running tests/unit/file_record_store.rs (<target>/debug/deps/unit_file_record_store-8a22a1ebe8b6b073)

running 9 tests
test cited_version_and_scrub::a_statements_edit_still_rebases_over_a_moved_head ... ok
test cited_version_and_scrub::an_import_commit_citing_another_version_is_a_conflict ... FAILED
test cited_version_and_scrub::a_re_import_citing_an_earlier_version_is_a_conflict ... FAILED
test cited_version_and_scrub::a_re_import_that_would_rebase_is_a_retryable_conflict_that_publishes_nothing ... FAILED
test cited_version_and_scrub::an_import_commit_that_would_rebase_is_a_retryable_conflict_that_publishes_nothing ... FAILED
test cited_version_and_scrub::the_store_refuses_to_import_a_scrubbed_record ... ok
test cited_version_and_scrub::the_store_refuses_to_append_an_entry_whose_source_name_is_withheld ... ok
test cited_version_and_scrub::an_import_commit_whose_cited_version_matches_publishes_it ... ok
test cited_version_and_scrub::the_store_refuses_to_save_a_scrubbed_record ... ok

failures:

---- cited_version_and_scrub::an_import_commit_citing_another_version_is_a_conflict stdout ----

thread 'cited_version_and_scrub::an_import_commit_citing_another_version_is_a_conflict' (31972) panicked at crates/metadata/tests/unit/file_record_store.rs:745:9:
Ok(Some(CommitReceipt { object_id: "urn:uuid:5a1f1b2e-0000-4000-8000-000000000001", version: VersionNum { number: "1", width: 0 }, created: "2026-10-21T01:12:08Z", inventory_digest: "ff0586ddf186f55a8efb92a760cac13db62593d309115a92d111583edccc6ea2", written: ["v1/content/original/5a1f1b2e-0000-4000-8000-000000000001.JPG", "v1/content/original/5a1f1b2e-0000-4000-8000-000000000001.metadata.md"], took_over: false }))
stack backtrace:
   0: __rustc::rust_begin_unwind
   1: core::panicking::panic_fmt
   2: unit_file_record_store::cited_version_and_scrub::an_import_commit_citing_another_version_is_a_conflict
   3: unit_file_record_store::cited_version_and_scrub::an_import_commit_citing_another_version_is_a_conflict::{closure#0}
   4: <unit_file_record_store::cited_version_and_scrub::an_import_commit_citing_another_version_is_a_conflict::{closure#0} as core::ops::function::FnOnce<()>>::call_once
note: Some details are omitted, run with `RUST_BACKTRACE=full` for a verbose backtrace.

---- cited_version_and_scrub::a_re_import_citing_an_earlier_version_is_a_conflict stdout ----

thread 'cited_version_and_scrub::a_re_import_citing_an_earlier_version_is_a_conflict' (31969) panicked at crates/metadata/tests/unit/file_record_store.rs:822:9:
Ok(Some(CommitReceipt { object_id: "urn:uuid:5a1f1b2e-0000-4000-8000-000000000001", version: VersionNum { number: "2", width: 0 }, created: "2026-10-21T01:12:08Z", inventory_digest: "c1f7c90f65613cac54565a3392f55e4abdd43b80d518a5a6cb679da2d1b7b05e", written: ["v2/content/original/5a1f1b2e-0000-4000-8000-000000000001.metadata.md"], took_over: false }))
stack backtrace:
   0: __rustc::rust_begin_unwind
   1: core::panicking::panic_fmt
   2: unit_file_record_store::cited_version_and_scrub::a_re_import_citing_an_earlier_version_is_a_conflict
   3: unit_file_record_store::cited_version_and_scrub::a_re_import_citing_an_earlier_version_is_a_conflict::{closure#0}
   4: <unit_file_record_store::cited_version_and_scrub::a_re_import_citing_an_earlier_version_is_a_conflict::{closure#0} as core::ops::function::FnOnce<()>>::call_once
note: Some details are omitted, run with `RUST_BACKTRACE=full` for a verbose backtrace.

---- cited_version_and_scrub::a_re_import_that_would_rebase_is_a_retryable_conflict_that_publishes_nothing stdout ----

thread 'cited_version_and_scrub::a_re_import_that_would_rebase_is_a_retryable_conflict_that_publishes_nothing' (31970) panicked at crates/metadata/tests/unit/file_record_store.rs:796:9:
Ok(Some(CommitReceipt { object_id: "urn:uuid:5a1f1b2e-0000-4000-8000-000000000001", version: VersionNum { number: "3", width: 0 }, created: "2026-10-21T01:12:08Z", inventory_digest: "52ae24c1be9ab637ceeb73dac335c715ec97b70355061007b9eea5095af78829", written: ["v3/content/original/5a1f1b2e-0000-4000-8000-000000000001.metadata.md"], took_over: false }))
stack backtrace:
   0: __rustc::rust_begin_unwind
   1: core::panicking::panic_fmt
   2: unit_file_record_store::cited_version_and_scrub::a_re_import_that_would_rebase_is_a_retryable_conflict_that_publishes_nothing
   3: unit_file_record_store::cited_version_and_scrub::a_re_import_that_would_rebase_is_a_retryable_conflict_that_publishes_nothing::{closure#0}
   4: <unit_file_record_store::cited_version_and_scrub::a_re_import_that_would_rebase_is_a_retryable_conflict_that_publishes_nothing::{closure#0} as core::ops::function::FnOnce<()>>::call_once
note: Some details are omitted, run with `RUST_BACKTRACE=full` for a verbose backtrace.

---- cited_version_and_scrub::an_import_commit_that_would_rebase_is_a_retryable_conflict_that_publishes_nothing stdout ----

thread 'cited_version_and_scrub::an_import_commit_that_would_rebase_is_a_retryable_conflict_that_publishes_nothing' (31973) panicked at crates/metadata/tests/unit/file_record_store.rs:763:9:
Ok(Some(CommitReceipt { object_id: "urn:uuid:5a1f1b2e-0000-4000-8000-000000000001", version: VersionNum { number: "3", width: 0 }, created: "2026-10-21T01:12:08Z", inventory_digest: "d4196fe3981c2b16823ddb5149dd9a82f0430ef0a244fa503d8c7a78170ea3c4", written: ["v3/content/original/5a1f1b2e-0000-4000-8000-000000000001.JPG", "v3/content/original/5a1f1b2e-0000-4000-8000-000000000001.metadata.md"], took_over: false }))
stack backtrace:
   0: __rustc::rust_begin_unwind
   1: core::panicking::panic_fmt
   2: unit_file_record_store::cited_version_and_scrub::an_import_commit_that_would_rebase_is_a_retryable_conflict_that_publishes_nothing
   3: unit_file_record_store::cited_version_and_scrub::an_import_commit_that_would_rebase_is_a_retryable_conflict_that_publishes_nothing::{closure#0}
   4: <unit_file_record_store::cited_version_and_scrub::an_import_commit_that_would_rebase_is_a_retryable_conflict_that_publishes_nothing::{closure#0} as core::ops::function::FnOnce<()>>::call_once
note: Some details are omitted, run with `RUST_BACKTRACE=full` for a verbose backtrace.


failures:
    cited_version_and_scrub::a_re_import_citing_an_earlier_version_is_a_conflict
    cited_version_and_scrub::a_re_import_that_would_rebase_is_a_retryable_conflict_that_publishes_nothing
    cited_version_and_scrub::an_import_commit_citing_another_version_is_a_conflict
    cited_version_and_scrub::an_import_commit_that_would_rebase_is_a_retryable_conflict_that_publishes_nothing

test result: FAILED. 5 passed; 4 failed; 0 ignored; 0 measured; 25 filtered out; finished in 0.08s

error: test failed, to rerun pass `-p filesquid-metadata --test unit_file_record_store`
```

### Green: every metadata test binary and the BDD features, after merging origin/main

`cargo test -p filesquid-metadata --all-features --locked`

```text
Finished `test` profile [unoptimized] target(s) in 5.73s
     Running unittests src/lib.rs (<target>/debug/deps/filesquid_metadata-f851fec434ee5da3)

running 3 tests
test tests::accrual_is_rejected_on_file ... ok
test tests::title_is_allowed ... ok
test tests::name_is_unknown ... ok

test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running tests/features.rs (<target>/debug/deps/features-08e86a57382d7750)
Feature: Reusable filesquid-metadata engine
  Scenario: Core API is namespaced metadata
fsquid metadata — metadata CRUD (via filesquid-ocfl)

Usage:
  fsquid <command> [args] [--object <dir>] [--id <object-id>]
  fsquid metadata <command> [args] [--object <dir>] [--id <object-id>]

Commands:
  extract <file>          Extract EXIF/ID3/XMP from a media file into metadata.md
  extract --file <logical-path>
                          Extract from a logical file of the object
  get [prefix:term]       Print populated statements (or one CURIE)
  set <prefix:term> <val> Set a statement and commit metadata.md
  export csv|xmp|json <path>  Write CSV, XMP or JSON sidecar interchange
  import csv|xmp|json <path>  Read CSV, XMP or JSON sidecar into metadata.md
  validate [prefix:term]  Check a term against the /terms/ register and the
                          namespace registry, or, without a term, every
                          statement of metadata.md; names each unregistered
                          prefix (always run as `fsquid metadata validate`)
  namespace register <prefix> <iri> [--terms a,b,...] [--custom]
                          Register an extension namespace (optional term
                          list) or, with --custom, declare a custom namespace
                          of free keys
  namespace list          Print the registered namespaces
  help                    Show this help

--object is an OCFL object directory (default `.`). --id names the object:
required to create one in a directory without an object, and checked
against an existing object's identifier.

--registry is the OCFL object directory that keeps the namespace registry
(namespaces.md); without it, the registry is the one in --object.
dcterms:, dc:, schema: and exif: are built in and cannot be registered.

JSON sidecars also take --file <logical-path>: the logical file of the
object's head whose SHA-256 digest binds the sidecar with the object
identifier. Import rejects a sidecar whose identifier or digest differs.

Canonical store: metadata.md (YAML front matter + Markdown body), a logical
file of the OCFL object. Each edit commits a new OCFL version; earlier
versions are never modified.
   ✔  When I inspect the filesquid-metadata public API
exported json to <tmp>/filesquid-metadata-features-32455-4.json
exported json to <tmp>/filesquid-metadata-features-32455-5.json
exported json to <tmp>/filesquid-metadata-features-32455-6.json
lido:title: namespace prefix lido is not registered
exported json to <tmp>/filesquid-metadata-features-32455-8.json
imported json from <tmp>/filesquid-metadata-features-32455-5.json: committed v2
   ✔  Then dcterms register and statement operations should be available
rico:title is a valid term
   ✔  And FileSquid catalog, recipe, C2PA, and collection-snapshot types should be absent
  Scenario: Unknown dcterms local names are rejected
   ✔  When I author dcterms:name on a file
   ✔  Then the write should be rejected
  Scenario: Accrual terms are rejected on a file
   ✔  When I author dcterms:accrualMethod on a file
   ✔  Then the write should be rejected
  Scenario: Canonical store is metadata.md
   ✔  When I set dcterms:title Harbour on an object
   ✔  Then metadata.md YAML front matter should contain dcterms title Harbour
   ✔  And catalogs should be rebuildable from that file
  Scenario: Each metadata edit is a new OCFL version
   ✔  Given an OCFL object with a logical file and dcterms:title Harbour
   ✔  When I set dcterms:title Dawn through the ocfl store
   ✔  Then the object should have a new version whose metadata.md holds dcterms title Dawn
   ✔  And the earlier versions should be unchanged
   ✔  And the object should pass the OCFL validator
  Scenario: An unchanged record publishes no version
   ✔  Given an OCFL object with a logical file and dcterms:title Harbour
   ✔  When I set dcterms:title Harbour through the ocfl store
   ✔  Then no version should be published
  Scenario: A caller's commit options go into the same OCFL version
   ✔  When I commit a metadata edit with a version user, an operation message and a version-bound record
   ✔  Then the edit should publish one version carrying that user, that message and the record
  Scenario: A file's one record lives beside its stored original in the same version
   ✔  When I import a file named "Holiday 2023.JPG" into an OCFL object
   ✔  Then the version should hold the stored original and its per-file metadata record
   ✔  And the object should hold no metadata.md
   ✔  And the imported object should pass the OCFL validator
  Scenario: The record opens with the original name, display name, extension, format and media type
   ✔  When I import a file named "Holiday 2023.JPG" into an OCFL object
   ✔  Then the record should open with the schema identifier and the original name Holiday 2023.JPG
   ✔  And the display name should be the original name
   ✔  And the extension, format and media type should be recorded
  Scenario: A re-import appends an entry in a new OCFL version
   ✔  When I import a file named "Holiday 2023.JPG" into an OCFL object
   ✔  And I import the same file again
   ✔  Then the object should have a new version with two import entries
   ✔  And the first version should be unchanged
  Scenario: A save that changes an import entry is refused
   ✔  When I import a file named "Holiday 2023.JPG" into an OCFL object
   ✔  And I save the record with a changed import entry
   ✔  Then the save should be refused for changing an import entry
   ✔  And the object's versions should be unchanged
  Scenario: A save that removes an import entry is refused
   ✔  When I import a file named "Holiday 2023.JPG" into an OCFL object
   ✔  And I import the same file again
   ✔  And I save the record with only the first import entry
   ✔  Then the save should be refused for removing an import entry
  Scenario: A save that changes the original file name is refused
   ✔  When I import a file named "Holiday 2023.JPG" into an OCFL object
   ✔  And I save the record with another original name
   ✔  Then the save should be refused for changing the original name
   ✔  And the object's versions should be unchanged
  Scenario: A rename changes only the display name
   ✔  When I import a file named "Holiday 2023.JPG" into an OCFL object
   ✔  And I rename the file to "Harbour.jpg"
   ✔  Then the display name should be Harbour.jpg and the original name Holiday 2023.JPG
   ✔  And the import entries should be unchanged
  Scenario: A statements edit keeps the header and the import section
   ✔  When I import a file named "Holiday 2023.JPG" into an OCFL object
   ✔  And I set dcterms:title Harbour on the file's record
   ✔  Then the record should hold dcterms title Harbour and still hold its original name and import entry
  Scenario: An import commit checks the version its entry cites
   ✔  When I import a file named "Holiday 2023.JPG" into an OCFL object
   ✔  And another writer commits a version before I import the same file again citing v2
   ✔  Then the re-import should be refused as a retryable conflict
   ✔  And the object should hold only the other writer's new version
   ✔  And a retry citing v3 should append the entry in v3
  Scenario: A statements edit still rebases over another writer's version
   ✔  When I import a file named "Holiday 2023.JPG" into an OCFL object
   ✔  And another writer commits a version while I set dcterms:title Harbour on the file's record
   ✔  Then the edit should be published as v3 on top of the other writer's version
  Scenario: The original file name is withheld from an outgoing copy by default
   ✔  When I import a file named "Holiday 2023.JPG" into an OCFL object
   ✔  And I make the outgoing copy of its record with every identity class
   ✔  Then its original name, display name and every source name should be withheld by policy
   ✔  And the store should refuse to save that copy
  Scenario: Use original filename keeps the names in the outgoing copy
   ✔  When I import a file named "Holiday 2023.JPG" into an OCFL object
   ✔  And I make the outgoing copy of its record with every identity class except the original name
   ✔  Then its original name, display name and source name should be Holiday 2023.JPG
  Scenario: A source size that differs from the verified size fails the import
   ✔  When I import a file whose reported size differs from the verified size
   ✔  Then the import should be rejected for the size mismatch
   ✔  And nothing should be published
  Scenario: Names and values that are not UTF-8 round-trip as base64
   ✔  When I import a file whose name and extended attribute are not UTF-8
   ✔  Then the record should store them as base64 marked with encoding base64
   ✔  And they should read back byte for byte
  Scenario: Every missing fact is listed as unavailable with a reason
   ✔  When I import a file through an upload that provides only a name, size, time and media type
   ✔  Then every other source, volume and attribute fact should be unavailable with the reason not provided by upload
  Scenario: Attribute inspection stops at 256 names and 1 MiB and marks what it cut
   ✔  When I inspect 300 extended attributes of 8 KiB each
   ✔  Then at most 256 names should be kept and the entry marked truncated with 300 names enumerated
   ✔  And at most 1 MiB of value bytes should be recorded as read
   ✔  And every value cut by the budget should have an incomplete digest
   ✔  And a re-import should get its own budget of 256 names and 1 MiB
  Scenario: A value over 64 KiB is kept as size and SHA-256 only
   ✔  When I inspect an extended attribute of 70000 bytes
   ✔  Then the record should hold its size and SHA-256 and not the value
  Scenario: The read projection omits paths, host and account
   ✔  When I import a file with a full set of source facts and read its projection
   ✔  Then the projection should show the original name, relative path, volume name and OCFL object
   ✔  And it should not show the absolute path, the import root, the host name or any account
  Scenario: The outgoing policy scrubs the identity fields
   ✔  When I scrub every identity class of a full import entry
   ✔  Then no path, account, host name, numeric ID, origin URL, quarantine agent or Finder comment should remain
   ✔  And each removed fact should be listed as withheld by policy
  Scenario: The import record builds for the browser without filesystem access
   ✔  When I inspect the import record modules
   ✔  Then they should use no filesystem, path or environment access
  Scenario: A head without its version inventory stays refused
   ✔  Given an OCFL object whose head has no version inventory
   ✔  When I set dcterms:title Dawn through the ocfl store
   ✔  Then the edit should be refused for the missing version inventory
   ✔  And no version should be published
   ✔  And the missing version inventory should not be repaired
  Scenario: Persistence is pluggable and filesquid-ocfl is optional
   ✔  When I inspect filesquid-metadata library dependencies
   ✔  Then filesquid-ocfl should be an optional dependency outside the default features
   ✔  And axum, tauri, libvips, and filesquid-core should be absent
  Scenario: CLI is the stand-alone UI
   ✔  When I ask the metadata command runner for help
   ✔  Then I should see extract, get, set, export, import, and validate
  Scenario: CSV headers are prefix:term
   ✔  When I import a CSV with headers id,dcterms:title,dcterms:name
   ✔  Then dcterms:name should be rejected
  Scenario: Interchange excludes MODS and SQL
   ✔  When I inspect filesquid-metadata interchange
   ✔  Then CSV, JSON sidecar, XMP sidecar, and embedded extract should be available
   ✔  And MODS and SQL should be absent
  Scenario: JSON sidecar binds statements to the object and file digest
   ✔  Given an OCFL object with a logical file and dcterms:title Harbour
   ✔  When I export the object's JSON sidecar
   ✔  Then the sidecar should be one UTF-8 JSON document for that object
   ✔  And it should carry the object identifier and the SHA-256 digest of the file
   ✔  And each statement should have a namespace, term, and value
  Scenario: JSON sidecar export then import reproduces the statements
   ✔  Given an OCFL object with a logical file and dcterms:title Harbour
   ✔  When I export the object's JSON sidecar
   ✔  And I import that sidecar into an empty object holding the same file
   ✔  Then the imported metadata.md should hold the same statements
  Scenario: JSON sidecar import rejects a digest mismatch
   ✔  Given an OCFL object with a logical file and dcterms:title Harbour
   ✔  When I export the object's JSON sidecar
   ✔  And I import that sidecar into an object whose file has different bytes
   ✔  Then the import should be rejected for a digest mismatch
   ✔  And that object's metadata.md should be unchanged
  Scenario: JSON sidecar import applies the /terms/ register
   ✔  When I import a JSON sidecar holding dcterms:name
   ✔  Then the JSON import should be rejected
  Scenario: JSON sidecar import rejects an unregistered namespace
   ✔  When I import a JSON sidecar holding rico:title
   ✔  Then the JSON import should be rejected naming the prefix rico
  Scenario: A distribution registers an extension namespace and authors a statement in it
   ✔  Given the namespace rico is registered with a term list
   ✔  When I author rico:title "Fonds 12" on a file
   ✔  Then metadata.md should hold rico title "Fonds 12" in its own map
   ✔  And no dcterms statement should be written
  Scenario: An owner declares a custom namespace of free keys
   ✔  Given the owner declares the custom namespace mine
   ✔  When I author mine:shelf_mark "B-7" on a file
   ✔  Then metadata.md should hold mine shelf_mark "B-7" in its own map
  Scenario Outline: A registration cannot redefine a built-in namespace
   ✔  When I register the namespace dcterms
   ✔  Then the registration should be refused naming dcterms
  Scenario Outline: A registration cannot redefine a built-in namespace
   ✔  When I register the namespace dc
   ✔  Then the registration should be refused naming dc
  Scenario Outline: A registration cannot redefine a built-in namespace
   ✔  When I register the namespace schema
   ✔  Then the registration should be refused naming schema
  Scenario Outline: A registration cannot redefine a built-in namespace
   ✔  When I register the namespace exif
   ✔  Then the registration should be refused naming exif
  Scenario: A prefix keeps its one IRI
   ✔  Given the namespace rico is registered with a term list
   ✔  When I register rico again with another IRI
   ✔  Then the registration should be refused naming rico
   ✔  And rico should keep its first IRI
  Scenario: A term list rejects unknown terms
   ✔  Given the namespace rico is registered with a term list
   ✔  When I author rico:colour "red" on a file
   ✔  Then the write should be rejected naming colour
  Scenario: A statement in an unregistered namespace is rejected by name
   ✔  Given the namespace rico is registered with a term list
   ✔  When I author lido:title "x" on a file
   ✔  Then the write should be rejected naming lido
  Scenario: Extension statements round-trip through metadata.md, CSV, JSON and XMP
   ✔  Given the namespace rico is registered with a term list
   ✔  When I author rico:title "Fonds 12" on a file
   ✔  Then the statement should round-trip unchanged through metadata.md, CSV, JSON and XMP
  Scenario: Registrations persist in canonical OCFL state
   ✔  Given the namespace rico is registered with a term list
   ✔  When I commit the registry to an OCFL object
   ✔  Then a store opened afresh over that object should recover the registration
   ✔  And the registry object should pass the OCFL validator
  Scenario: The CLI validate command reports an unregistered prefix
   ✔  Given an OCFL object whose registry holds rico
   ✔  When I validate lido:title with the CLI
   ✔  Then validate should fail
   ✔  When I validate rico:title with the CLI
   ✔  Then validate should succeed
  Scenario: JSON sidecar does not replace metadata.md
   ✔  Given an OCFL object with a logical file and dcterms:title Harbour
   ✔  When I export the object's JSON sidecar
   ✔  Then metadata.md should remain the canonical store in the object
  Scenario: Stand-alone FOSS product
   ✔  When I inspect filesquid-metadata product identity
   ✔  Then it should remain installable without the FileSquid app
   ✔  And it should build and publish without FileSquid application crates
  Scenario: Engine builds for the browser
   ✔  When I build filesquid-metadata for wasm32-unknown-unknown with default features off
   ✔  Then the build should succeed
  Scenario: Engine builds for the browser with the OCFL store
   ✔  When I build filesquid-metadata for wasm32-unknown-unknown with the ocfl feature
   ✔  Then the build should succeed
  Scenario: Extraction reads bytes from filesquid-ocfl
   ✔  When I extract metadata from a logical file in an OCFL object
   ✔  Then the extractor should read bytes supplied by filesquid-ocfl, not a filesystem path
  Scenario: Extraction maps an embedded XMP packet onto dcterms
   ✔  When I extract metadata from bytes holding an XMP packet with dc:title Harbour
   ✔  Then the extracted dcterms:title should be Harbour
  Scenario: Extraction takes a reader and a file reference, independent of any store
   ✔  When I inspect the filesquid-metadata extraction API
   ✔  Then extraction should take bytes or a reader plus a FileRef and no path
  Scenario: Sidecar store keeps a record beside a plain file
   ✔  Given a plain file in a directory
   ✔  When I save a record through the sidecar store
   ✔  Then the record should be loadable from the sidecar beside the file
  Scenario: CLI and path helpers are native-only
   ✔  When I inspect the filesquid-metadata native-only modules
   ✔  Then the CLI, the sidecar store and the OCFL path helpers should be compiled out for wasm32
  Scenario: Metadata panel plugs into a file browser sidebar
   ?  Given the filesquid-ocfl file browser with a selected file
      Step skipped: tests/features/reusable_metadata_engine_F-052.feature:308:5
[Summary]
1 feature
59 scenarios (58 passed, 1 skipped)
181 steps (180 passed, 1 skipped)
     Running tests/unit/cli.rs (<target>/debug/deps/unit_cli-fca5a15484d2aaf5)

running 31 tests
test csv_export_still_ignores_json_flags ... ok
test export_csv_contains_header ... ok
test export_xmp_writes_the_head_record ... ok
test a_registered_extension_statement_is_set_in_its_own_map ... ok
test a_separate_registry_object_is_named_with_registry ... ok
test extract_reads_a_logical_file_through_filesquid_ocfl ... ok
test extract_reads_a_plain_file_through_a_reader_into_metadata_md ... ok
test extract_refuses_a_path_and_a_logical_file_together ... ok
test help_lists_json_interchange ... ok
test help_prints_commands ... ok
test extract_reports_a_missing_file ... ok
test import_csv_and_xmp_commit_new_versions ... ok
test extension_statements_round_trip_through_cli_json ... ok
test json_import_rejects_other_object_id ... ok
test json_rejects_a_file_the_object_does_not_hold ... ok
test json_rejects_a_flag_as_the_id_value ... ok
test json_requires_file ... ok
test extension_statements_round_trip_through_cli_csv_and_xmp ... ok
test namespace_subcommands_refuse_extra_operands ... ok
test json_import_rejects_digest_mismatch_and_publishes_nothing ... ok
test json_export_then_import_round_trips_through_metadata_md ... ok
test set_on_a_new_object_needs_an_identifier ... ok
test set_keeps_json_flag_names_as_literal_values ... ok
test unknown_command_errors ... ok
test validate_name_fails ... ok
test namespace_register_refuses_a_built_in_prefix_and_another_iri ... ok
test set_rejects_an_unregistered_prefix_or_an_unlisted_term_by_name ... ok
test validate_title_ok ... ok
test set_and_get_round_trip_with_one_version_per_edit ... ok
test validate_reads_a_named_registry_without_an_object ... ok
test validate_reports_unregistered_prefixes ... ok

test result: ok. 31 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.14s

     Running tests/unit/csv.rs (<target>/debug/deps/unit_csv-5bd695e62e5a615c)

running 10 tests
test dcterms_title_header ... ok
test quoted_cell_maps_to_its_own_column ... ok
test line_skips_id_column ... ok
test physically_blank_lines_are_skipped ... ok
test quoted_whitespace_value_is_kept ... ok
test round_trip_keeps_commas ... ok
test unknown_dcterms_header_rejected ... ok
test round_trip_keeps_quotes ... ok
test round_trip_keeps_newlines ... ok
test unterminated_quote_is_rejected ... ok

test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running tests/unit/extract.rs (<target>/debug/deps/unit_extract-cb8ec81a5ddb769e)

running 13 tests
test a_name_that_only_starts_like_xmpmeta_is_not_a_packet ... ok
test a_reader_can_be_extracted_twice ... ok
test an_embedded_xmp_packet_maps_onto_dcterms ... ok
test an_unterminated_xmp_packet_is_not_an_error ... ok
test bytes_without_an_xmp_packet_report_none ... ok
test bytes_without_tags_extract_nothing_and_are_not_an_error ... ok
test an_unterminated_cdata_section_is_kept_as_written ... ok
test exif_from_bytes_sets_native_and_dcterms_statements ... ok
test truncated_tag_blocks_are_not_an_error ... ok
test whitespace_before_attributes_does_not_end_the_scan ... ok
test id3_from_a_reader_maps_title_and_artist ... ok
test xmp_values_decode_every_entity_reference_and_cdata ... ok
test an_unclosed_element_ends_the_scan_without_an_error ... ok

test result: ok. 13 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running tests/unit/file_record_store.rs (<target>/debug/deps/unit_file_record_store-45934ecbe5501a6e)

running 34 tests
test a_failed_import_leaves_its_change_set_so_a_retry_can_publish ... ok
test a_plain_metadata_md_is_unchanged_by_the_per_file_path ... ok
test a_non_utf8_name_survives_the_store ... ok
test a_re_import_must_cite_content_the_object_holds ... ok
test a_re_import_appends_an_entry_in_a_new_version_and_leaves_v1_untouched ... ok
test a_record_path_must_be_a_valid_logical_path ... ok
test a_record_saved_at_the_content_path_is_refused ... ok
test a_re_import_still_commits_the_callers_bound_files ... ok
test a_re_import_uses_the_callers_commit_options ... ok
test a_referenced_import_commits_its_record_without_content ... ok
test a_save_that_changes_an_import_entry_is_refused_and_publishes_nothing ... ok
test a_re_import_whose_original_is_replaced_before_its_commit_is_a_conflict ... ok
test a_save_that_changes_the_original_name_is_refused ... ok
test a_statements_only_save_without_a_file_record_is_refused ... ok
test an_entry_must_cite_the_object_and_the_path_of_the_file_it_is_committed_for ... ok
test an_import_without_its_staged_content_is_refused ... ok
test a_rename_is_a_new_version_that_changes_only_the_display_name ... ok
test appending_to_a_missing_record_is_refused ... ok
test a_statements_only_save_keeps_the_header_and_every_import_entry ... ok
test an_unchanged_record_publishes_nothing ... ok
test a_save_that_removes_an_import_entry_is_refused ... ok
test cited_version_and_scrub::a_re_import_citing_an_earlier_version_is_a_conflict ... ok
test cited_version_and_scrub::an_import_commit_citing_another_version_is_a_conflict ... ok
test cited_version_and_scrub::an_import_commit_whose_cited_version_matches_publishes_it ... ok
test cited_version_and_scrub::a_statements_edit_still_rebases_over_a_moved_head ... ok
test cited_version_and_scrub::the_store_refuses_to_append_an_entry_whose_source_name_is_withheld ... ok
test cited_version_and_scrub::a_re_import_that_would_rebase_is_a_retryable_conflict_that_publishes_nothing ... ok
test cited_version_and_scrub::the_store_refuses_to_import_a_scrubbed_record ... ok
test cited_version_and_scrub::an_import_commit_that_would_rebase_is_a_retryable_conflict_that_publishes_nothing ... ok
test staged_content_that_is_not_the_verified_content_is_refused ... ok
test cited_version_and_scrub::the_store_refuses_to_save_a_scrubbed_record ... ok
test the_store_type_is_usable_with_the_default_commit_helper ... ok
test the_record_reads_back_exactly ... ok
test the_record_lives_beside_the_original_in_the_same_version_as_its_content ... ok

test result: ok. 34 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.05s

     Running tests/unit/file_ref.rs (<target>/debug/deps/unit_file_ref-a62b5dba5ec281fe)

running 3 tests
test accessors_return_what_the_caller_supplied ... ok
test version_is_optional_and_object_id_may_be_empty ... ok
test escaping_or_empty_logical_paths_are_rejected ... ok

test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running tests/unit/import_record.rs (<target>/debug/deps/unit_import_record-7a4d4c280bde0e84)

running 70 tests
test attribute_bounds::a_value_cut_short_has_an_incomplete_digest ... ok
test attribute_bounds::a_value_up_to_64_kib_is_stored_whole ... ok
test attribute_bounds::a_non_utf8_name_and_value_are_stored_as_base64 ... ok
test attribute_bounds::a_hand_built_entry_over_a_bound_is_rejected ... ok
test attribute_bounds::dropping_names_without_marking_truncation_is_rejected ... ok
test attribute_bounds::the_257th_name_is_refused_and_the_entry_is_marked_truncated ... ok
test entry_validation::a_managed_import_cites_its_logical_path_and_a_referenced_one_does_not ... ok
test entry_validation::an_unavailable_fact_serializes_with_its_reason ... ok
test entry_validation::differing_reported_and_verified_sizes_are_rejected ... ok
test entry_validation::an_unavailable_reported_size_is_allowed ... ok
test entry_validation::every_time_field_accepts_valid_timestamps ... ok
test attribute_bounds::a_larger_value_is_kept_as_size_and_digest_only ... ok
test entry_validation::the_cited_digest_is_64_lowercase_hex_digits ... ok
test entry_validation::the_cited_size_must_be_the_verified_size ... ok
test entry_validation::the_upload_profile_is_valid_and_lists_what_an_upload_cannot_know ... ok
test file_record::a_missing_header_field_or_a_broken_entry_is_refused ... ok
test entry_validation::json_round_trips_and_every_field_must_be_present ... ok
test file_record::a_new_record_starts_with_the_original_name_as_its_display_name ... ok
test file_record::a_re_import_appends_and_leaves_earlier_entries_unchanged ... ok
test file_record::a_record_without_an_import_entry_is_invalid ... ok
test file_record::a_non_utf8_original_name_round_trips_exactly ... ok
test file_record::a_rename_changes_only_the_display_name ... ok
test file_record::a_save_that_changes_an_import_entry_is_refused ... ok
test file_record::a_save_that_changes_the_original_name_is_refused ... ok
test file_record::a_save_that_removes_an_import_entry_is_refused ... ok
test file_record::a_wrong_or_missing_schema_is_refused ... ok
test file_record::names_and_types_are_validated ... ok
test file_record::statements_and_the_display_name_may_change ... ok
test entry_validation::every_time_field_must_be_rfc_3339_utc ... ok
test file_record::markdown_round_trips_statements_body_and_imports ... ok
test names::the_extension_is_what_follows_the_last_dot ... ok
test file_record::the_markdown_opens_with_the_schema_and_the_names ... ok
test file_record::with_statements_keeps_the_header_and_the_imports ... ok
test names::the_record_name_is_the_file_uuid_plus_metadata_md ... ok
test names::the_stored_extension_is_one_to_sixteen_ascii_letters_or_digits ... ok
test original_name_class::a_header_name_unavailable_for_another_reason_is_malformed ... ok
test original_name_class::a_record_with_only_one_header_name_withheld_is_malformed ... ok
test original_name_class::a_record_scrub_withholds_the_header_names_and_every_source_name ... ok
test original_name_class::a_record_scrub_applies_the_other_classes_to_every_entry ... ok
test original_name_class::a_renamed_display_name_is_withheld_too ... ok
test original_name_class::a_scrubbed_record_cannot_be_renamed ... ok
test original_name_class::a_scrub_without_the_original_name_keeps_every_name ... ok
test original_name_class::an_entry_scrub_withholds_the_source_name ... ok
test original_name_class::a_source_name_is_known_and_never_empty_or_unavailable_for_another_reason ... ok
test original_name_class::every_class_includes_the_original_name ... ok
test projection_and_scrub::each_identity_class_reaches_its_own_fields ... ok
test projection_and_scrub::known_attributes_are_withheld_by_their_class_and_keep_name_and_size ... ok
test projection_and_scrub::scrubbing_a_class_withholds_exactly_its_fields ... ok
test projection_and_scrub::scrubbing_every_class_leaves_no_identity_value ... ok
test original_name_class::a_scrubbed_record_reads_back_as_scrubbed ... ok
test raw_values::base64_follows_rfc_4648_vectors ... ok
test projection_and_scrub::the_projection_names_the_file_and_the_import_but_no_location_or_account ... ok
test raw_values::every_byte_value_round_trips ... ok
test raw_values::invalid_base64_or_an_unknown_encoding_is_refused ... ok
test raw_values::other_bytes_are_base64_marked_with_their_encoding ... ok
test raw_values::utf8_is_a_plain_string ... ok
test review_fixes::a_raw_envelope_with_a_duplicate_field_is_refused ... ok
test review_fixes::a_managed_entry_cites_a_valid_logical_path ... ok
test review_fixes::an_upload_keeps_a_utf8_extension_of_a_non_utf8_name ... ok
test review_fixes::an_extension_unrelated_to_the_original_name_is_refused ... ok
test review_fixes::an_upload_lists_the_missing_uti_as_unavailable_with_a_reason ... ok
test review_fixes::a_line_after_import_that_is_not_an_entry_is_refused_not_dropped ... ok
test review_fixes::schema_org_statements_do_not_collide_with_the_record_schema_key ... ok
test review_fixes::stored_whole_values_cannot_exceed_the_bytes_the_entry_says_it_read ... ok
test review_fixes_three::a_digest_is_only_kept_for_a_value_over_64_kib ... ok
test review_fixes_two::a_refused_257th_name_still_marks_the_entry_truncated ... ok
test review_fixes_two::a_whole_value_must_be_its_declared_size_and_a_digest_must_be_sha256_hex ... ok
test review_fixes::the_display_name_is_transformed_once_for_the_projection ... ok
test attribute_bounds::the_read_budget_is_one_mebibyte_across_the_entry ... ok
test review_fixes::each_acquisition_gets_its_own_attribute_budget ... ok

test result: ok. 70 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s

     Running tests/unit/isolation.rs (<target>/debug/deps/unit_isolation-2ad164b104aef085)

running 4 tests
test default_features_exclude_filesquid_ocfl ... ok
test excludes_hosts_codecs_and_filesquid_core ... ok
test filesquid_ocfl_is_an_optional_dependency ... ok
test the_ocfl_feature_enables_the_optional_dependency ... ok

test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running tests/unit/json.rs (<target>/debug/deps/unit_json-5f5532124da20da9)

running 19 tests
test digest_algorithm_is_sha256 ... ok
test array_of_objects_is_rejected ... ok
test export_is_utf8_without_escaping_unicode ... ok
test export_binds_object_id_and_digest ... ok
test export_writes_namespace_term_value_statements ... ok
test export_then_import_reproduces_statements ... ok
test import_accepts_uppercase_digest_hex ... ok
test import_merges_into_existing_record ... ok
test import_maps_dc_onto_dcterms ... ok
test import_rejects_accrual_term_on_file ... ok
test import_rejects_digest_mismatch ... ok
test import_rejects_other_digest_algorithm ... ok
test import_rejects_unknown_dcterms_term ... ok
test import_rejects_other_object_id ... ok
test import_rejects_unregistered_namespace_by_name ... ok
test malformed_json_is_rejected ... ok
test missing_digest_is_rejected ... ok
test sha256_matches_fips_vector ... ok
test rejected_import_leaves_record_unchanged ... ok

test result: ok. 19 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running tests/unit/markdown.rs (<target>/debug/deps/unit_markdown-d013e68ff32a4bec)

running 3 tests
test line_breaks_survive_metadata_md ... ok
test quotes_and_backslashes_survive_metadata_md ... ok
test yaml_front_matter_round_trip ... ok

test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running tests/unit/namespace.rs (<target>/debug/deps/unit_namespace-514c5d7779995504)

running 47 tests
test csv::a_csv_header_outside_a_term_list_is_rejected ... ok
test csv::a_csv_header_with_an_unregistered_prefix_is_rejected_by_name ... ok
test csv::extension_statements_round_trip_through_csv ... ok
test csv::a_csv_header_line_resolves_registered_prefixes ... ok
test markdown::a_blank_line_inside_a_map_keeps_the_map_open ... ok
test json::json_import_rejects_a_term_outside_the_list ... ok
test json::extension_statements_round_trip_through_json ... ok
test json::json_import_applies_the_registry ... ok
test markdown::a_map_of_an_unregistered_prefix_is_kept_on_read ... ok
test markdown::each_extension_prefix_is_its_own_yaml_map ... ok
test markdown::only_indented_lines_belong_to_a_map ... ok
test markdown::extension_statements_round_trip_unchanged ... ok
test persistence::a_stored_registration_that_breaks_a_rule_is_refused_on_read ... ok
test persistence::a_malformed_registry_record_is_refused ... ok
test persistence::a_registry_save_may_only_add ... ok
test persistence::the_registry_round_trips_through_its_markdown_record ... ok
test registration::a_built_in_iri_cannot_be_registered_under_another_prefix ... ok
test registration::a_custom_namespace_cannot_become_an_extension ... ok
test registration::a_distribution_registers_an_extension_with_a_term_list ... ok
test registration::a_module_registers_an_extension_without_a_term_list ... ok
test registration::an_identical_re_registration_changes_nothing ... ok
test registration::a_term_list_cannot_be_dropped_and_an_open_namespace_cannot_gain_one ... ok
test registration::an_iri_belongs_to_one_prefix ... ok
test registration::an_owner_declares_a_custom_namespace_of_free_keys ... ok
test registration::duplicate_terms_in_a_list_are_refused ... ok
test registration::re_registering_a_prefix_with_another_iri_is_rejected ... ok
test registration::the_built_in_namespaces_cannot_be_redefined ... ok
test registration::the_record_format_keys_cannot_be_prefixes ... ok
test statements::a_curie_resolves_a_registered_prefix_and_names_an_unregistered_one ... ok
test registration::the_same_iri_may_add_terms_but_never_remove_them ... ok
test registration::prefixes_terms_and_iris_must_be_well_formed ... ok
test statements::a_custom_key_must_be_a_valid_term_name ... ok
test statements::a_term_list_rejects_unknown_terms ... ok
test statements::an_unregistered_prefix_is_rejected_by_name ... ok
test statements::an_extension_statement_is_never_a_dcterms_statement ... ok
test statements::the_registry_keeps_the_dcterms_register_and_the_dc_mapping ... ok
test statements::without_a_registry_an_extension_statement_is_unregistered ... ok
test xmp::xmp_export_rejects_a_term_that_is_not_an_element_name ... ok
test xmp::xmp_export_rejects_an_unregistered_prefix_by_name ... ok
test xmp::extension_statements_round_trip_through_xmp ... ok
test xmp::xmp_import_reads_a_namespace_by_its_iri_under_any_packet_prefix ... ok
test xmp::xmp_import_reads_declarations_only_from_start_tags ... ok
test xmp::xmp_import_reads_every_packet_prefix_of_a_registered_iri_in_one_pass ... ok
test xmp::xmp_import_reads_registered_properties_nested_in_another ... ok
test xmp::xmp_import_skips_a_packet_prefix_bound_to_two_iris ... ok
test xmp::xmp_import_tokenizes_quoted_attributes_and_processing_instructions ... ok
test registration::registrations_and_term_lists_are_bounded ... ok

test result: ok. 47 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s

     Running tests/unit/namespace_store.rs (<target>/debug/deps/unit_namespace_store-80bfb797b0c3af3d)

running 6 tests
test an_existing_object_without_a_registry_record_reads_as_empty ... ok
test a_save_that_drops_a_registration_is_refused_and_publishes_nothing ... ok
test an_object_without_a_registry_reads_as_empty ... ok
test a_registration_is_committed_in_one_version_and_read_back ... ok
test an_unchanged_registry_publishes_nothing ... ok
test a_new_registration_is_a_new_version ... ok

test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s

     Running tests/unit/register.rs (<target>/debug/deps/unit_register-293af9f1979c216e)

running 10 tests
test close_and_unmatched_terms_do_not_alias ... ok
test creator_is_accepted ... ok
test dc_title_maps_to_dcterms_title ... ok
test one_to_one_map_has_22_rows_of_registered_terms ... ok
test one_to_one_map_is_injective_both_ways ... ok
test one_to_one_map_matches_expected_table ... ok
test renamed_equivalents_map ... ok
test unknown_dc_name_does_not_map ... ok
test title_maps_to_schema_name_both_ways ... ok
test unknown_term_is_rejected ... ok

test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running tests/unit/sidecar.rs (<target>/debug/deps/unit_sidecar-5462564e5b35473b)

running 8 tests
test a_file_without_a_sidecar_loads_an_empty_record ... ok
test a_sidecar_that_is_not_utf8_is_an_error_not_a_lossy_record ... ok
test a_git_working_tree_is_refused ... ok
test a_missing_parent_directory_is_a_backend_error ... ok
test a_symlinked_directory_cannot_leave_the_root ... ok
test save_then_load_round_trips_beside_the_file ... ok
test a_symlinked_staging_file_cannot_redirect_the_write ... ok
test save_replaces_the_earlier_record ... ok

test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running tests/unit/store.rs (<target>/debug/deps/unit_store-eab858085c3416e1)

running 28 tests
test a_committed_record_that_is_not_utf8_is_an_error ... ok
test a_missing_object_or_metadata_md_is_an_empty_record ... ok
test a_csv_line_break_survives_commit_and_load ... ok
test a_head_without_its_version_inventory_stays_refused ... ok
test an_object_root_that_could_escape_is_an_invalid_path ... ok
test a_stale_change_set_of_this_writer_is_discarded_before_staging ... ok
test an_unchanged_record_publishes_nothing ... ok
test an_edit_never_modifies_an_earlier_version ... ok
test a_pinned_version_that_is_not_the_head_is_a_conflict ... ok
test commit_options::a_failing_bound_file_function_publishes_nothing ... ok
test commit_options::a_no_op_save_commits_nothing_and_binds_no_files ... ok
test commit_options::user_message_and_bound_files_are_committed_in_the_same_version ... ok
test commit_options::without_options_the_store_keeps_its_stand_alone_defaults ... ok
test local_object::a_git_working_tree_is_refused ... ok
test local_object::a_new_object_directory_needs_an_identifier ... ok
test file_sha256_hashes_the_logical_file_through_filesquid_ocfl ... ok
test local_object::another_identifier_is_refused ... ok
test each_edit_commits_exactly_one_new_version ... ok
test local_object::each_save_is_a_new_version_of_a_valid_object ... ok
test metadata_md_is_a_logical_file_inside_a_version ... ok
test local_object::an_edit_from_a_head_another_writer_moved_is_a_conflict ... ok
test local_object::the_digest_of_a_missing_logical_file_is_an_error ... ok
test reading_a_root_that_holds_another_object_is_refused ... ok
test staging_is_cleared_after_each_edit ... ok
test the_first_edit_of_a_new_object_creates_v1 ... ok
test per_edit_versions_on_a_filesystem_backend ... ok
test two_local_objects_in_one_process_stage_apart ... ok
test records_larger_than_one_put_round_trip ... ok

test result: ok. 28 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.99s

   Doc-tests filesquid_metadata

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```
