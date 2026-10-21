# Task DE-1.2 Walkthrough: File and folder import

Branch: `feat/de-1.2-file-folder-import` (part of global [Task 1.2](../../../../../../resources/implementation-plan.md#task-1.2)). API contract `0.21.0` (one above `0.20.0` of API-F1.13, PR #111).

## What changed

- **`filesquid-api`**:
  - `POST /api/v1/imports` (`files.write`; [`imports.rs`](../../../api/src/imports.rs), API-S-036):
    - the body is a `CommandEnvelope` of schema version 1 (operation ID = idempotency key, scope `library:<id>`), the library (non-empty) and the `source` (`files` or `folder`);
    - the reserved `collection_id` is refused with `400 invalid_request` and the journal code `collection_not_supported`, before any dialog, so no caller believes its files were filed;
    - the host port `ImportStarter`, installed with `ApiState::install_import_starter`;
    - answers the `CommandOutcome` API-S-002 requires, beside `import`: `pending` with `import: started` and the job ID, or `committed` with `import: cancelled` and no job;
    - refusals: `404 not_found` (`library_not_found`), `409 conflict` (`dialog_open`), `503 unavailable` (also on `filesquid-server`, which has no starter);
    - each request is journaled once as `import.start`, never with a path.
  - [`FileService::import_original`](../../../api/src/files/import.rs) is the one import commit. A host hands it the bytes, the gathered `SourceImport` facts and the file ID it minted when it planned the import. The commit:
    - writes the new file object exactly as the API-F1.4 upload does, in one version: `original/<uuid>.<ext>`, or `original/<uuid>` when the source has no extension or one that is not 1 to 16 ASCII letters or digits, and `original/<uuid>.metadata.md` with the import entry;
    - answers the object an earlier run of the same job already committed under that ID, so a run repeated after a crash never imports a file twice, and refuses an object of any other job or upload as a `conflict`;
    - refuses bytes whose length is not the stated size (`size_mismatch`).

    An import only ever creates version v1 of a new object, so the version its entry cites is the one committed.
  - `openapi.json` is 0.21.0, with schemas `ImportRequest`, `ImportAnswer`, `ImportSource` and `ImportOutcome`. `ui/src/api/schema.d.ts` was regenerated.
- **`filesquid-native`** (`desktop` feature; [`import/`](../../../native/src/import/mod.rs)):
  - `facts`, `xattr`, `volume`, `host`, `names` and `time` gather the import-record facts before the file is read, using `libc` and crates already in the lockfile only:
    - stat facts and times in nanoseconds (a time outside 1970 to 9999 is unavailable, never misformatted);
    - owner and group names (`getpwuid_r`, `getgrgid_r`);
    - extended attributes (`listxattr`/`getxattr`): one whose size cannot be read is left out and the entry marked truncated; `com.apple.quarantine`, `kMDItemWhereFroms`, `_kMDItemUserTags`, `kMDItemFinderComment` and `com.apple.FinderInfo` are decoded, the property lists through `plist` `=1.10.1` from the lockfile;
    - the volume from `/proc/self/mountinfo`: the deepest mount point above the file among those of its device, with the common network file systems marked network;
    - `uname`, the host name and the account, gathered before the file is read like the source facts.
  - `file` imports one source:
    - the source's and the host's facts first, then one bounded read, then `import_original`;
    - a system clock outside 1970 to 9999 fails the file (`clock_unavailable`) rather than record a false start time;
    - a file above 512 MiB is refused as `too_large_for_this_build`, which counts as retryable for a later build but is not retried within the job.
  - `plans` (`SqliteImportPlans`, in `filesquid.sqlite`) keeps:
    - each job's plan, with its sources one per row (Unix bytes, or Windows UTF-16 units, so every path round-trips);
    - its manifest;
    - whether it finished.

    The operation ID is unique, so a repeated operation finds its job. Read-then-write transactions take the write lock first (`BEGIN IMMEDIATE`), so the journal's writes to the same file make them wait rather than fail.
  - `job` runs one import as a durable job:
    - the first run lists the sources once in a stable, bounded order (100,000 entries, 64 levels, no links to folders) and keeps that listing as the manifest, each file with a minted file ID;
    - nothing that cannot be listed is dropped: it stays in the manifest with its code and is quarantined in turn;
    - more than 100,000 entries fail the job (`too_many_files`) before any import;
    - checkpoint `{"next": N}` per manifest entry, so a resumed run continues the same files whatever changed on disk;
    - 5 attempts with backoff capped at 60 s plus jitter, then quarantine, counted in the run's `retries`;
    - a failed lease renewal stops the run;
    - the staging cap and the free-space reserve, with suspension when space runs short or at quit.
  - `worker` (`ImportWorker`): one thread with a current-thread Tokio runtime built before the thread starts, and a queue of at most 64 jobs. When none is queued, it pages through the unfinished plans 64 at a time, so every unfinished job is tried once per launch however many there are. A queued job without a plan is failed (`plan_missing`).
  - New direct optional dependencies, all already in `Cargo.lock` at their locked versions, so the lockfile gains no crate: `icu_normalizer` `=2.3.0` (`compiled_data`), `plist` `=1.10.1` and `tokio` (`rt`).
- **`filesquid-desktop`**:
  - [`imports.rs`](../../src/imports.rs): `DesktopImports` implements `ImportStarter`. It:
    - answers a known operation's job without a dialog;
    - refuses a library that is not open;
    - shows one `rfd` dialog at a time (`RfdSources`, parented to the main window);
    - then, under one lock, checks the operation again, submits the job, saves its plan (failing the job, `plan_unsaved`, when it cannot) and queues it.
  - [`lib.rs`](../../src/lib.rs) starts the import worker at launch, after the backfill route. The worker runs unfinished imports when the launch recovered the job store. The app's exit waits for the worker, bounded at 10 s.
- **Docs**: API SRS API-S-036; desktop SRS DE-S-019; README import section; desktop plan status and the new Task DE-1.7 (also in the global plan's Task 1.2 and the checklist).

## Decisions (orchestrator, 2026-10-20)

- **Route:** option A, `POST /api/v1/imports` with the host's `rfd` picker, an `ImportStarter` port, one durable job per pick, `503` on the server and a minor contract bump. Under the owner's "follow SRS" decision (API-S-002) the import takes a `CommandEnvelope`, and re-submitting an operation ID answers the same job. A cancelled pick makes no job.
- **Name form:** detected with `icu_normalizer` 2.3.0, already in `Cargo.lock` (no `unicode-normalization`). The order is:
  - a name that is NFC and NFD at once (ASCII included) is recorded as `NFC`, as the upload does;
  - otherwise, the first form the name is in of NFD, NFC, NFKC and NFKD;
  - otherwise, `unnormalised`.
- **Facts:** `libc` only, plus crates already in the lockfile. Date added, ACL and the content type are recorded as unavailable with a reason that says which case applies:
  - on macOS, "kept by macOS; not read by this build", because reading them needs macOS frameworks;
  - on Linux, "not provided by this platform".

  Outside Linux and macOS, every Unix and extended-attribute fact, the host name, the account and the system version are unavailable too. Windows lookups would need `windows-sys` calls this decision leaves out.
- **Interim stage:** each file is read once into memory, hashed and checked against its size, and staged through `stage_bytes`. Files above **512 MiB** are refused with the retryable `too_large_for_this_build`. The planned streamed stage is a small OCFL modules task, to be dispatched with IO-1.6 and MD-0.10:

  ```rust
  ChangeModel::stage_file<D: StorageBackend>(&self, path: &str, source: &D, key: &str) -> Result<String, ChangeError>
  ```

  Once it lands, the desktop swaps it in and the limit goes.

## Known limits

- **PROV-O:** the import's PROV-O record joins its version through API-F2.9 and DE-0.10 (Task FS-2.8); this task leaves the hook.
- **Catalog indexing (orchestrator decision 2026-10-20, option B):** imports go into libraries the file service serves, which are the folder-bound libraries of DE-1.1. Indexing imported files into a catalog library's collections, with their logical paths and object IDs, is the new [Task DE-1.7](../plan.md) (12 hours, under global Task 1.2). That task needs:
  - the catalog's roots served by the file service;
  - the logical path and object ID in the collection snapshot, so the catalog stays rebuildable;
  - a catalog schema migration;
  - a default collection for a request without `collection_id`.

  Until then, a request naming a `collection_id` is refused (`collection_not_supported`). DE-1.7 overlaps the open question of uploaded files having no home collection or folder; the owner will be asked about the two library models (catalog libraries and folder-bound libraries). `CatalogFolders` and `CatalogScopes` (DE-1.1) still imply no folder and no scope object; per-file objects alone would not change that, because a scope object holds the files of a folder at logical paths.
- **Per-resource attempt caps (from DE-0.4):** DE-0.4 left these caps (5 attempts, 60 s backoff, NFR-003) to the resources of import and upload.
  - Done here: each file of an import has its own cap of 5 attempts, with backoff capped at 60 s plus jitter, and is then quarantined while the batch goes on (`a_file_that_keeps_failing_is_quarantined_after_five_attempts_and_the_rest_go_on`).
  - Unchanged: the job's own `attempts` still counts its leases, and a suspended import job still resumes at every launch without job-level quarantine.
  - Not applicable: an API upload is one request, which its client retries.
- **Packaged window:** WebDriver cannot drive a native dialog, so the import is covered through the scheme with a scripted dialog. The packaged-window e2e has no import spec yet; it would need a test-only dialog like DE-S-017's environment variable.

## Review rounds

| Round | Findings | What changed | Evidence |
|---|---|---|---|
| Cubic 1 (40: 8 P1, 29 P2, 3 P3) | Recovery capped at 64 jobs; plan-less jobs left pending; job and plan not atomic; checkpoint index over a mutable listing; lease renewal ignored; unreadable entries dropped; commit not idempotent with its checkpoint; failed sources discarded | Paged recovery from the plans; `plan_missing` and `plan_unsaved` failures; one start lock; the job manifest with minted file IDs and kept skipped entries; `Lost` on a failed renewal; the import commit creates the manifest's object and answers a same-job replay | `the_worker_runs_every_unfinished_job_beyond_one_queue_full`, `a_queued_job_without_a_plan_is_failed_not_left_pending`, `what_cannot_be_listed_is_kept_with_its_code`, `a_quit_suspends_between_files_and_the_resume_imports_only_the_rest`, `a_file_committed_before_a_crash_is_answered_not_imported_twice` |
| | Mutation answer not a `CommandOutcome`; envelope schema version and empty library ID unchecked | `ImportAnswer` carries the `CommandOutcome` (`pending` or `committed`) beside `import`; both refused as `invalid_request` | `unit_imports_route`, `unit_contract` |
| | Facts: failed attribute read recorded as empty; macOS where-from, tags, comment and colour label not decoded; network types; mount by device only; link-read and metadata errors misclassified; out-of-range times; clock before 1970; host facts after the read | Each fixed as listed under What changed; the macOS-only reasons say "not read by this build" | `unit_import_facts` (13 tests) |
| | Tests: no checkpoint, digest, object or path, or build assertions; quarantine not retried; no quit between files; dialog count for an unopened library | Each assertion added; the quarantine fixture is a retryable conflict | `unit_import_job`, `unit_import_file`, the feature |
| | Docs: plan hours; conditional extension; README contract test | F2 subtotal 520; desktop total recomputed (598 hours); walkthrough and README updated | this file, the README |
| | Declined: Windows host facts | Outside the libc-only decision; recorded unavailable with a reason | DE-S-019 |

A further fix found while testing: the plans' read-then-write transactions now take the write lock first (`BEGIN IMMEDIATE`), because the journal writes to the same SQLite file and a deferred transaction failed at once with "database is locked".

## Test results

Run from `apps/filesquid` with the shared build lock. Raw output:

```text
$ cargo test -p filesquid-api --locked --test unit_imports_route --test unit_contract
     Running tests/unit/contract.rs
running 28 tests
test error_code_all_lists_every_variant ... ok
test command_envelope_carries_idempotency_key_expected_revision_and_scope ... ok
test error_codes_match_the_rust_enum_with_status_and_retryability ... ok
test errors_carry_code_and_retryability ... ok
test event_topics_in_the_contract_match_the_rust_topics ... ok
test contract_is_openapi_3_1_with_the_crate_api_version ... ok
test authentication_before_routing_is_documented_as_a_wire_convention ... ok
test every_component_schema_has_a_rust_sample ... ok
test every_documented_operation_has_a_unique_operation_id ... ok
test every_component_schema_matches_its_serialized_rust_type ... ok
test every_error_response_carries_the_api_error_schema ... ok
test every_event_topic_documents_its_payload_schema ... ok
test every_operation_documents_both_denials ... ok
test every_limit_parameter_is_the_shared_page_size ... ok
test every_operation_names_the_action_the_router_authorises ... ok
test every_operation_requires_the_bearer_scheme_and_none_makes_it_optional ... ok
test health_schema_matches_the_core_payload_and_requires_every_field ... ok
test query_envelope_and_page_size_parameter_are_bounded_like_core ... ok
test host_routes_are_the_event_channel_only ... ok
test routed_and_documented_paths_agree_both_ways ... ok
test router_fallbacks_are_documented_as_wire_conventions ... ok
test router_serves_its_own_contract ... ok
test the_checker_accepts_only_truly_unconstrained_objects ... ok
test the_contract_uses_only_checked_schema_keywords ... ok
test the_only_public_operation_is_the_liveness_probe ... ok
test events_from_every_publisher_match_their_documented_payloads ... ok
test every_documented_operation_answers_a_documented_status_and_body ... ok
test router_fallbacks_answer_as_documented ... ok
test result: ok. 28 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.71s
     Running tests/unit/imports_route.rs
running 5 tests
test a_started_import_answers_its_job_and_journals_it_once ... ok
test a_cancelled_dialog_answers_cancelled_without_a_job ... ok
test a_collection_is_refused_before_any_dialog ... ok
test refusals_are_typed_and_journaled_once_with_their_code ... ok
test a_wrong_scope_no_starter_or_no_token_asks_nothing ... ok
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s

$ cargo test -p filesquid-native --features desktop --locked --test unit_import_facts --test unit_import_file --test unit_import_job
     Running tests/unit/import_facts.rs
running 13 tests
test a_folder_or_a_missing_file_is_refused ... ok
test a_large_attribute_is_kept_by_size_and_digest ... ok
test a_fixture_reports_its_known_times_size_owner_and_mode ... ok
test a_symbolic_link_records_its_target_and_the_targets_size ... ok
test the_deepest_mount_above_the_file_is_chosen_and_network_types_are_known ... ok
test the_host_names_this_build_and_its_system ... ok
test name_forms_are_told_apart ... ok
test facts_the_platform_cannot_give_are_listed_with_a_reason ... ok
test extended_attributes_are_read_with_their_values ... ok
test times_outside_what_rfc_3339_states_are_refused ... ok
test the_quarantine_attribute_is_decoded ... ok
test the_mount_table_gives_device_point_and_type ... ok
test where_from_tags_comment_and_colour_label_are_decoded ... ok
test result: ok. 13 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/unit/import_file.rs
running 4 tests
test a_missing_source_is_refused_without_retry ... ok
test a_file_above_the_in_memory_bound_is_refused_before_it_is_read ... ok
test a_size_that_is_not_the_size_read_is_refused_and_nothing_is_published ... ok
test an_import_stores_the_original_under_its_uuid_with_its_record_in_one_version ... ok
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
     Running tests/unit/import_job.rs
running 12 tests
test a_folder_is_imported_as_one_job_with_a_checkpoint_per_file ... ok
test a_file_too_large_for_this_build_is_quarantined_without_retries ... ok
test a_folder_is_listed_in_one_stable_order_with_its_subfolders ... ok
test a_plan_and_its_manifest_survive_a_restart_and_one_operation_has_one_job ... ok
test a_file_committed_before_a_crash_is_answered_not_imported_twice ... ok
test checkpoints_backoff_and_reserve_follow_their_rules ... ok
test picked_files_are_imported_each_with_itself_as_its_root ... ok
test a_file_that_keeps_failing_is_quarantined_after_five_attempts_and_the_rest_go_on ... ok
test what_cannot_be_listed_is_kept_with_its_code ... ok
test a_queued_job_without_a_plan_is_failed_not_left_pending ... ok
test a_quit_suspends_between_files_and_the_resume_imports_only_the_rest ... ok
test the_worker_runs_every_unfinished_job_beyond_one_queue_full ... ok
test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.93s

$ cargo test -p filesquid-desktop --locked --test integration_imports
     Running tests/integration/imports.rs
running 3 tests
test a_cancelled_dialog_or_an_unopened_library_makes_no_job ... ok
test a_picked_folder_is_imported_as_one_job_and_a_repeat_answers_the_same_job ... ok
test a_job_the_stopped_worker_could_not_take_runs_at_the_next_launch ... ok
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.25s

$ cargo test -p filesquid-desktop --features test-webdriver --locked --test features
Feature: Desktop file and folder import
  Scenario: A picked folder is imported as one job, one object per file
   ✔> Given the desktop host has started its import route over an open library
   ✔  Given the owner will pick a folder of 2 files
   ✔  When the app asks to import a folder under the operation "op-holidays"
   ✔  Then the answer should name a started import job
   ✔  And 2 files should be imported, each as its own object
   ✔  And no journal entry should name a picked file or folder
  Scenario: Repeating an operation answers the same job without a second dialog
   ✔> Given the desktop host has started its import route over an open library
   ✔  Given the owner will pick a folder of 2 files
   ✔  When the app asks to import a folder under the operation "op-holidays"
   ✔  And the app asks to import a folder under the operation "op-holidays" again
   ✔  Then both answers should name the same import job
   ✔  And the dialog should have been shown once
   ✔  And 2 files should be imported, each as its own object
  Scenario: A cancelled dialog makes no job
   ✔> Given the desktop host has started its import route over an open library
   ✔  Given the owner will close the dialog without picking anything
   ✔  When the app asks to import files under the operation "op-cancel"
   ✔  Then the answer should say the import was cancelled
   ✔  And no file should be imported
  Scenario: A library that is not open is refused and journaled once without a path
   ✔> Given the desktop host has started its import route over an open library
   ✔  Given the owner will pick a folder of 2 files
   ✔  When the app asks to import into a library that is not open
   ✔  Then the import should be refused as not found
   ✔  And the dialog should not have been shown
   ✔  And the journal should record one import request without any path
  Scenario: An import the host could not run now runs at the next launch
   ✔> Given the desktop host has started its import route over an open library
   ✔  Given the owner will pick a folder of 2 files
   ✔  And the import worker has stopped for the app to quit
   ✔  When the app asks to import a folder under the operation "op-later"
   ✔  Then the import should be refused as unavailable
   ✔  And after the app restarts 2 files should be imported, each as its own object
[Summary]
5 features
42 scenarios (42 passed)
174 steps (174 passed)
```

The contract tests (`unit_contract`) check version 0.21.0 and the import schemas' samples. The authorisation table (`unit_auth`, `/api/v1/imports` under `files.write`) also passes. The full pre-commit run is recorded in the pull request.

## SRS mapping

| Requirement | Title | Evidence |
|---|---|---|
| [SRS-F-002](../../../../../../resources/srs.md#srs-f-002-safe-file-folder-and-device-import) | Safe file, folder and device import | `desktop_imports_F-002.feature`; `integration_imports`; `unit_import_job` |
| [API-S-036](../../../api/resources/srs.md) | Import route | `unit_imports_route` (started as `pending`, cancelled as `committed`, typed refusals journaled once, a collection refused before any dialog, wrong scope, schema version or library, no starter, no token); `unit_contract`; `unit_auth` |
| [API-S-035](../../../api/resources/srs.md) | Metadata property routes (the upload's object layout and import record, reused by the import commit) | `an_import_stores_the_original_under_its_uuid_with_its_record_in_one_version`, `a_size_that_is_not_the_size_read_is_refused_and_nothing_is_published` |
| [API-S-033](../../../api/resources/srs.md) | Durable job model | `a_folder_is_imported_as_one_job_with_a_checkpoint_per_file`, `a_quit_suspends_between_files_and_the_resume_imports_only_the_rest`, `the_worker_runs_every_unfinished_job_beyond_one_queue_full`, `a_file_committed_before_a_crash_is_answered_not_imported_twice` |
| [DE-S-019](../srs.md) | Desktop file and folder import | every test above; facts in `unit_import_facts` (known times, size, owner and mode; extended attributes; large attribute bounds; quarantine, where-from, tag, comment and colour-label decoding; deepest mount and network types; time bounds; link target; mount table; name forms; unavailable facts with reasons; host facts) |
| [DE-S-017](../srs.md) | Native library folder choice (the dialog rule the import dialog follows) | `RfdSources` parented to the main window; scripted dialog in `integration_imports` |
| [DE-S-015](../srs.md) | Native-processing library boundary | `integration_native_boundary` (the import worker and its storage live in `filesquid-native`) |
| [DE-S-006](../srs.md) | The webview MUST receive opaque handles and API responses, never raw filesystem paths or SQL | path and name assertions on every answer and journal entry in `integration_imports` and the feature |
| [SRS-NFR-003](../../../../../../resources/srs.md#srs-nfr-003-bounded-processing-and-migration-endurance) | Bounded processing and migration endurance | `a_file_that_keeps_failing_is_quarantined_after_five_attempts_and_the_rest_go_on`, `checkpoints_backoff_and_reserve_follow_their_rules`, `a_file_above_the_in_memory_bound_is_refused_before_it_is_read` |
