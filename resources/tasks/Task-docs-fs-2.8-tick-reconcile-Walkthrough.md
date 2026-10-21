# Task docs-fs-2.8-tick-reconcile Walkthrough: reconcile the plan boxes of global Task FS-2.8

Branch: `docs/fs-2.8-tick-implemented-rows`. Scope: documentation only (plan and checklist boxes and Status notes; no code, SRS, ADR, PRD or rule file).

## Why the boxes were open

[PR #86](https://github.com/arkrim-tech/filesquid/pull/86) (`feat/prov-o-history`, "FS-2.8 Rust rows") delivered CO-0.11, IO-1.5, PV-0.1, PV-0.2, MD-0.9 and API-F2.8 together with their walkthroughs, but no commit of the PR ticked a plan or checklist box. No walkthrough, plan Status note or commit message says a box was left open on purpose. The [FS-2.8 host wiring walkthrough](../../apps/filesquid/crates/desktop/resources/tasks/Task-FS-2.8-host-wiring-Walkthrough.md) separately explains why API-F2.9, DE-0.10 and global FS-2.8 stay open.

## Rows

A row is ticked only when its Description is delivered, its listed dependencies are ticked and the tests its Agent Prompt requires exist. No Rust tests were added for this documentation change (the pre-commit hook still ran the existing ones, see Verification), so a missing test keeps a row open with a Status note.

| Row | Result | Evidence |
|---|---|---|
| PV-0.1 | Ticked | [Walkthrough](../../apps/filesquid/crates/prov/resources/tasks/Task-PV-0.1-Walkthrough.md): crate in the workspace members, model, JSON-LD profile with bounded parser, builder, history index, isolation tests and the `wasm32-unknown-unknown` check; every test the Agent Prompt names is listed. No dependencies. |
| IO-1.5 | Left open | Description delivered ([Walkthrough](../../apps/filesquid/crates/ocfl/resources/tasks/Task-IO-1.5-Walkthrough.md); `commit_bound`, `commit_with`, `revert_with`, `reinstate_with`, `commit_bound_only` on main). Missing: the OCFL-S-018 write-order test for bound files on `FsBackend` (`tests/integration/bound_files.rs` and the BDD steps use `MemoryBackend` only), which the Agent Prompt requires. |
| PV-0.2 | Left open | Description delivered ([Walkthrough](../../apps/filesquid/crates/prov/resources/tasks/Task-PV-0.2-Walkthrough.md); `read_record`, `list_records`, `RecordHook`, `reconstruct` in `crates/prov/src/ocfl.rs`). Missing: tests against `FsBackend` (`tests/unit/ocfl.rs` uses `MemoryBackend` only); dependency IO-1.5 is open. |
| MD-0.9 | Left open | Description delivered ([Walkthrough](../../apps/filesquid/crates/metadata/resources/tasks/Task-MD-0.9-Walkthrough.md); `CommitOptions`, `commit_record_with` returning the receipt). Missing: the "rebased edit regenerates its bound file" test (no rebase test in `crates/metadata/tests`); dependency IO-1.5 is open. MD-0.8, which the walkthrough recorded as not done, is ticked on main. |
| CO-0.11 | Left open | `OperationMessage`, `VersionRef`, `InstallationId`, `JournalLink`, `HistoryEntry`, the extended `ProvOSidecar` and the corrected `PROV_O_CONTEXT` are delivered. `OperationKind` (21 kinds) has no `publish`, which the Description lists and the F-067 feature uses; `CatalogOrganisation` is written `catalog` where the F-067 feature says `catalog.organisation`; `reconstruction` is spelled `backfill`. |
| API-F2.8 | Left open | Delivered: `operation_id` and `object_ref` on `JournalEntry` with redaction, `ocfl.commit` entries, pre-allocated IDs, takeover entries, repair run ID, snapshot and restore commits through `CommitRecorder`. Missing on the file routes (API-F1.3) and metadata routes (API-F1.4, both on main): the installation `user`, the `filesquid.operation.v1` message and `ocfl.commit` (they pass the caller as `user`; metadata routes use `FileSquid <operation>`, `files/statements.rs`), and the `metadata.edit` journal entry. The walkthrough's "Not delivered" section predates those routes. |
| API-F2.9, DE-0.10, global FS-2.8 | Not touched | As instructed. |

The checklist matches: PV-0.1 is ticked, the other five stay open. The test gaps are small and need Rust (an `FsBackend` run of the bound-file and PROV-O tests, and a metadata rebase test); once they exist, IO-1.5, PV-0.2 and MD-0.9 can be ticked. The owner may instead accept the ticks without them.

## Global Task 0.8 (not changed)

Its only listed module row, UI-0.6, is ticked, but the global box needs more. Left to do by the global Description, none of it found on main: `resources/tasks/Task-0.8-Walkthrough.md` does not exist; the item-level F-040 capability register (supported, limited and unsupported rows for the Photos baseline and every F, NFR and TC ID, F-047 and F-048 pending, F-050 to F-060 against the phases) has no document (only architecture and PRD text describe it); the proposed quantitative budgets are recorded only as architecture text; the TC-004 statement of architecture versus scaffold reality; and the optional OTLP profile, which the README does not mention (the journal and the Monitor page are documented in README paragraphs; the Description asks for them together with the OTLP profile in one place). BE-0.6 moved to FS-3.10, and its F-040, NFR-002 and NFR-008 features live under `crates/server/tests/features/`. The owner decides whether the global box closes with those items or is narrowed.

## Verification

Raw CLI output (the hook's status lines from the last full run, 308 s, taken from its log; the full log is about 13,500 lines of cargo test output).

```text
$ python3 scripts/docs/update_mvp_progress.py
Wrote resources/implementation-plan-checklist.md
$ bash scripts/pre-commit.sh
✅ Branch Validation passed.
✅ ./apps/filesquid/ui uniquely bound to 5173
✅ ./apps/filesquid/ui uniquely bound to 4173
✅ ./apps/imagesquid uniquely bound to 5174
✅ ./apps/imagesquid uniquely bound to 4174
✅ filesquid-server uniquely bound to 8083
✅ Traceability Validation passed: 96 PRD requirements, 84 resolved SRS references.
✅ Workspace Boundary validation passed.
✅ Logging rule validation passed (no relevant staged files).
✅ ./apps/filesquid is properly formatted.
✅ ./apps/filesquid/ui is properly formatted.
✅ ./apps/imagesquid is properly formatted.
✅ Format validation passed.
✅ ADR structure (ISO 42010) passed.
✅ PRD allocation passed.
✅ Plan traceability passed.
✅ Relative links and anchors passed.
✅ FileSquid and ImageSquid document boundary passed.
✅ MVP queue and merge driver tests passed.
✅ MVP progress bar passed.
✅ Task cleanup passed.
✅ Documentation validation passed.
✅ Change scope validation passed (168 assertions).
ℹ️  Test scope: rust:filesquid-api,filesquid-cli,filesquid-core,filesquid-desktop,filesquid-metadata,filesquid-native,filesquid-ocfl,filesquid-prov,filesquid-server
ℹ️  Rust scope: filesquid-api filesquid-cli filesquid-core filesquid-desktop filesquid-metadata filesquid-native filesquid-ocfl filesquid-prov filesquid-server
✅ Clippy (scoped) passed (1s).
✅ Scoped tests passed (136s).
✅ Module crate alone: filesquid-core passed (0s).
✅ Module crate alone: filesquid-ocfl passed (23s).
✅ Module crate alone: filesquid-metadata passed (0s).
✅ Module crate alone: filesquid-prov passed (1s).
✅ Module crate alone: filesquid-api passed (89s).
✅ Module crate alone: filesquid-native passed (3s).
✅ Clippy (filesquid-metadata, all features) passed (0s).
✅ Tests (filesquid-metadata, all features) passed (5s).
✅ wasm32 build (filesquid-metadata, default features off) passed (0s).
✅ wasm32 build (filesquid-metadata, ocfl feature) passed (0s).
✅ Clippy (filesquid-prov, all features) passed (0s).
✅ Tests (filesquid-prov, all features) passed (1s).
✅ wasm32 build (filesquid-prov) passed (1s).
✅ wasm32 build (filesquid-prov, ocfl feature) passed (0s).
✅ Rustdoc (scoped) passed (4s).
✅ Clippy (desktop, test-instrumented) passed (0s).
✅ Desktop tests (test-instrumented) passed (23s).
ℹ️  Desktop packaged-window e2e skipped (set FILESQUID_DESKTOP_E2E=1 to run).
✅ Lint and test validation passed (test scope: rust:filesquid-api,filesquid-cli,filesquid-core,filesquid-desktop,filesquid-metadata,filesquid-native,filesquid-ocfl,filesquid-prov,filesquid-server).
✅ All Pre-Commit Hooks Passed! Test scope: rust:filesquid-api,filesquid-cli,filesquid-core,filesquid-desktop,filesquid-metadata,filesquid-native,filesquid-ocfl,filesquid-prov,filesquid-server (308s)
```

The scope was the nine Rust crates, not docs: the hook counts a change under `crates/<crate>/resources/` as a change to that crate (`scripts/verify_changed_scope.sh`). No code changed.
