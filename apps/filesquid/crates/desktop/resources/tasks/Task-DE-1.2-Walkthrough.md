# Task DE-1.2 Walkthrough: File and folder import

Branch: `feat/de-1.2-file-folder-import` (part of global [Task 1.2](../../../../../../resources/implementation-plan.md#task-1.2)). API contract `0.20.0` (one above `0.19.0` of DE-1.1, PR #109).

## What changed

- **`filesquid-api`**:
  - `POST /api/v1/imports` (`files.write`; [`imports.rs`](../../../api/src/imports.rs), API-S-036):
    - the body is a `CommandEnvelope` (operation ID = idempotency key, scope `library:<id>`), the library and the `source` (`files` or `folder`);
    - the reserved `collection_id` is refused with `400 invalid_request` and the journal code `collection_not_supported`, before any dialog, so no caller believes its files were filed;
    - the host port `ImportStarter`, installed with `ApiState::install_import_starter`;
    - answers `started` with the job ID, or `cancelled` with no job;
    - refusals: `404 not_found` (`library_not_found`), `409 conflict` (`dialog_open`), `503 unavailable` (also on `filesquid-server`, which has no starter);
    - each request is journaled once as `import.start`, never with a path.
  - [`FileService::import_original`](../../../api/src/files/import.rs) is the one import commit. A host hands it the bytes and the gathered `SourceImport` facts, and it writes the file's own object exactly as the API-F1.4 upload does: `original/<uuid>.<ext>` and `original/<uuid>.metadata.md` with the import entry, in one version. It refuses bytes whose length is not the stated size (`size_mismatch`).
  - `openapi.json` is 0.20.0, with schemas `ImportRequest`, `ImportAnswer`, `ImportSource` and `ImportOutcome`. `ui/src/api/schema.d.ts` was regenerated.
- **`filesquid-native`** (`desktop` feature; [`import/`](../../../native/src/import/mod.rs)):
  - `facts`, `xattr`, `volume`, `host`, `names` and `time` gather the import-record facts before the file is read, using `libc` only:
    - stat facts and times in nanoseconds;
    - owner and group names (`getpwuid_r`, `getgrgid_r`);
    - extended attributes (`listxattr`/`getxattr`, with `com.apple.quarantine` decoded);
    - the volume from `/proc/self/mountinfo`;
    - `uname`, the host name and the account.
  - `file` imports one source:
    - facts first, then one bounded read, then `import_original`;
    - a file above 512 MiB is refused as `too_large_for_this_build`, which counts as retryable for a later build but is not retried within the job.
  - `plans` (`SqliteImportPlans`, in `filesquid.sqlite`) keeps each job's plan. The operation ID is unique, so a repeated operation finds its job.
  - `job` runs one import as a durable job:
    - stable bounded listing (100,000 files, 64 levels, no links to folders);
    - checkpoint `{"next": N}` per file;
    - 5 attempts with backoff capped at 60 s plus jitter, then quarantine;
    - the staging cap and the free-space reserve, with suspension when space runs short or at quit.
  - `worker` (`ImportWorker`): one thread with a current-thread Tokio runtime, a queue of at most 64 jobs, and `resume_unfinished` at launch.
  - New direct optional dependencies, both already in `Cargo.lock` at their locked versions, so the lockfile gains no crate: `icu_normalizer` `=2.3.0` (`compiled_data`) and `tokio` (`rt`).
- **`filesquid-desktop`**:
  - [`imports.rs`](../../src/imports.rs): `DesktopImports` implements `ImportStarter`. It answers a known operation's job without a dialog, refuses a library that is not open, shows one `rfd` dialog at a time (`RfdSources`, parented to the main window), then saves the plan, submits the job and queues it.
  - [`lib.rs`](../../src/lib.rs) starts the import worker at launch, after the backfill route, and resumes unfinished imports when the launch recovered jobs. The app's exit waits for the worker, bounded at 10 s.
- **Docs**: API SRS API-S-036; desktop SRS DE-S-019; README import section; desktop plan status and the new Task DE-1.7 (also in the global plan's Task 1.2 and the checklist).

## Decisions (orchestrator, 2026-10-20)

- **Route:** option A, `POST /api/v1/imports` with the host's `rfd` picker, an `ImportStarter` port, one durable job per pick, `503` on the server and a minor contract bump. Under the owner's "follow SRS" decision (API-S-002) the import takes a `CommandEnvelope`, and re-submitting an operation ID answers the same job. A cancelled pick makes no job.
- **Name form:** detected with `icu_normalizer` 2.3.0, already in `Cargo.lock` (no `unicode-normalization`). The order is:
  - a name that is NFC and NFD at once (ASCII included) is recorded as `NFC`, as the upload does;
  - otherwise, the first form the name is in of NFD, NFC, NFKC and NFKD;
  - otherwise, `unnormalised`.
- **Facts:** `libc` only. Date added, ACL and the content type are recorded as unavailable with a reason on Linux; every fact outside Linux and macOS is too.
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

## Test results

Run from `apps/filesquid` with the shared build lock. Raw output:

```text
$ cargo test -p filesquid-api --locked --test unit_imports_route
     Running tests/unit/imports_route.rs
running 5 tests
test a_collection_is_refused_before_any_dialog ... ok
test a_cancelled_dialog_answers_cancelled_without_a_job ... ok
test a_started_import_answers_its_job_and_journals_it_once ... ok
test a_wrong_scope_no_starter_or_no_token_asks_nothing ... ok
test refusals_are_typed_and_journaled_once_with_their_code ... ok
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s

$ cargo test -p filesquid-native --features desktop --locked --test unit_import_facts --test unit_import_file --test unit_import_job
     Running tests/unit/import_facts.rs
running 10 tests
test a_folder_or_a_missing_file_is_refused ... ok
test a_large_attribute_is_kept_by_size_and_digest ... ok
test a_symbolic_link_records_its_target_and_the_targets_size ... ok
test extended_attributes_are_read_with_their_values ... ok
test a_fixture_reports_its_known_times_size_owner_and_mode ... ok
test name_forms_are_told_apart ... ok
test the_host_names_this_build_and_its_system ... ok
test the_mount_table_gives_device_point_and_type ... ok
test the_quarantine_attribute_is_decoded ... ok
test facts_the_platform_cannot_give_are_listed_with_a_reason ... ok
test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/unit/import_file.rs
running 4 tests
test a_missing_source_is_refused_without_retry ... ok
test a_file_above_the_in_memory_bound_is_refused_before_it_is_read ... ok
test a_size_that_is_not_the_size_read_is_refused_and_nothing_is_published ... ok
test an_import_stores_the_original_under_its_uuid_with_its_record_in_one_version ... ok
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
     Running tests/unit/import_job.rs
running 8 tests
test a_folder_is_listed_in_one_stable_order_with_its_subfolders ... ok
test a_plan_survives_a_restart_and_one_operation_has_one_job ... ok
test checkpoints_backoff_and_reserve_follow_their_rules ... ok
test a_quit_suspends_between_files_and_the_resume_skips_what_was_done ... ok
test picked_files_are_imported_each_with_itself_as_its_root ... ok
test a_file_that_keeps_failing_is_quarantined_after_five_attempts_and_the_rest_go_on ... ok
test a_folder_is_imported_as_one_job_with_a_checkpoint_per_file ... ok
test the_worker_runs_queued_jobs_from_their_plans_and_resumes_unfinished_ones ... ok
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.15s

$ cargo test -p filesquid-desktop --locked --test integration_imports
     Running tests/integration/imports.rs
running 3 tests
test a_cancelled_dialog_or_an_unopened_library_makes_no_job ... ok
test a_picked_folder_is_imported_as_one_job_and_a_repeat_answers_the_same_job ... ok
test a_job_the_stopped_worker_could_not_take_runs_at_the_next_launch ... ok
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.16s

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
173 steps (173 passed)
```

The API contract tests (`unit_contract`, version 0.20.0 and the import schemas' samples) and the authorisation table (`unit_auth`, `/api/v1/imports` under `files.write`) also pass. The full pre-commit run is recorded in the pull request.

## SRS mapping

| Requirement | Title | Evidence |
|---|---|---|
| [SRS-F-002](../../../../../../resources/srs.md#srs-f-002-safe-file-folder-and-device-import) | Safe file, folder and device import | `desktop_imports_F-002.feature`; `integration_imports`; `unit_import_job` |
| [API-S-036](../../../api/resources/srs.md) | Import route | `unit_imports_route` (started, cancelled, typed refusals journaled once, a collection refused before any dialog, wrong scope, no starter, no token); `unit_contract`; `unit_auth` |
| [API-S-035](../../../api/resources/srs.md) | Metadata property routes (the upload's object layout and import record, reused by the import commit) | `an_import_stores_the_original_under_its_uuid_with_its_record_in_one_version`, `a_size_that_is_not_the_size_read_is_refused_and_nothing_is_published` |
| [API-S-033](../../../api/resources/srs.md) | Durable job model | `a_folder_is_imported_as_one_job_with_a_checkpoint_per_file`, `a_quit_suspends_between_files_and_the_resume_skips_what_was_done`, `the_worker_runs_queued_jobs_from_their_plans_and_resumes_unfinished_ones` |
| [DE-S-019](../srs.md) | Desktop file and folder import | every test above; facts in `unit_import_facts` (known times, size, owner and mode; extended attributes; large attribute bounds; quarantine decoding; link target; mount table; name forms; unavailable facts with reasons; host facts) |
| [DE-S-017](../srs.md) | Native library folder choice (the dialog rule the import dialog follows) | `RfdSources` parented to the main window; scripted dialog in `integration_imports` |
| [DE-S-015](../srs.md) | Native-processing library boundary | `integration_native_boundary` (the import worker and its storage live in `filesquid-native`) |
| [DE-S-006](../srs.md) | The webview MUST receive opaque handles and API responses, never raw filesystem paths or SQL | path and name assertions on every answer and journal entry in `integration_imports` and the feature |
| [SRS-NFR-003](../../../../../../resources/srs.md#srs-nfr-003-bounded-processing-and-migration-endurance) | Bounded processing and migration endurance | `a_file_that_keeps_failing_is_quarantined_after_five_attempts_and_the_rest_go_on`, `checkpoints_backoff_and_reserve_follow_their_rules`, `a_file_above_the_in_memory_bound_is_refused_before_it_is_read` |
