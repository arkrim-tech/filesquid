# Walkthrough: record the owner decisions of 2026-10-20

Branch: `docs/record-owner-decisions-2026-10-20`. Documentation only: no Rust, TypeScript or gate script changed. TDD-ENFORCE: Not applicable - documentation update.

## The decision

The owner's decision, verbatim, dated 2026-10-20:

> "1: A+3a. 2: (a) keep, (b) check, (e) add class. 3: A. 4: follow-ups. 5: follow SRS."

| Item | Question | Choice | Where it is recorded |
|---|---|---|---|
| 1 | File metadata record format: `filesquid-api`'s four-field `<file-uuid>.metadata.md` against `filesquid-metadata`'s `FileRecord` (API-F1.4 conflict artifact `Task-API-F1.4-Conflict.md` on branch `feat/api-f1.4-metadata-property-routes`) | A + 3a: `FileRecord` is the only codec; API-S-026 matches ADR 038; API-F1.4 takes the stored-name half of API-F1.13, so an upload creates `files/<file-id>/original/<uuid>.<ext>` with its record from the start (`.<ext>` only when the original extension is 1 to 16 ASCII letters or digits; otherwise the original is stored as `<uuid>` with no suffix) | [ADR 038](../architecture/038-import-file-record.md) Status and Decision 8; [API-S-026](../../apps/filesquid/crates/api/resources/srs.md); API-F1.4 and API-F1.13 in the [api plan](../../apps/filesquid/crates/api/resources/plan.md); Task FS-1.3 in the [implementation plan](../implementation-plan.md#task-fs-1.3) |
| 2(a) | Schema.org statements in a file record | keep `schemaorg:` (read from either key) | ADR 038 Decision 9; [MD-S-008 and MD-S-012](../../apps/filesquid/crates/metadata/resources/srs.md) |
| 2(b) | Cited `object_version` of an import entry | check inside the commit; a rebase becomes a retryable conflict for import commits, an approved exception to MD-S-008 | ADR 038 Decision 10; MD-S-008; new Task MD-0.10 |
| 2(e) | Original file name in the scrub | add the `OriginalName` identity class, on by default for shares, off when the saved policy selects **Use original filename** | ADR 038 Decision 6; [MD-S-013](../../apps/filesquid/crates/metadata/resources/srs.md); [SRS-F-046](../srs.md#srs-f-046-media-metadata-inspection-correction-and-privacy-exports); [PRD F-046](../prd.md#f-046-media-metadata-inspection-correction-and-privacy-exports); new Task MD-0.10; API-F1.13 |
| 3 | Where a `fsquid` run journals (API-F2.10 conflict artifact `Task-API-F2.10-Conflict.md`, sections 1 to 5, branch `feat/api-f2.10-client-events-journal-export`, PR #97) | option A | [ADR 036](../architecture/036-single-fsquid-cli.md) Status and Decision 8; [API-S-031](../../apps/filesquid/crates/api/resources/srs.md); [SRS-F-061](../srs.md#srs-f-061-activity-journal-and-live-health-monitoring); [PRD F-061](../prd.md#f-061-activity-journal-and-live-health-monitoring); root README `## View logs` (as planned behaviour); new Task CLI-0.2 |
| 4 | Engine warnings without full context (section 6 of the same artifact) | follow-ups, no new journal exception | new [OCFL-S-025](../../apps/filesquid/crates/ocfl/resources/srs.md); API-S-028 and API-S-031; new Tasks IO-1.6 and API-F2.12 |
| 5 | File-write contract (API-F2.2 conflict artifact `Task-API-F2.2-Conflict.md` on branch `feat/api-f2.2-views-copy-move-zip`, option A) | follow the SRS: UI-S-012 and the contract's convention win | [API-S-002](../../apps/filesquid/crates/api/resources/srs.md); new Task API-F2.13 |

`logging-enforce.md` keeps its two exceptions; no exception was granted. `.agents/rules/`, `AGENTS.md` and the gate scripts are unchanged.

## New tasks

| Task | Plan | Global task | Hours |
|---|---|---|---|
| MD-0.10 Cited-version check for import commits and the `OriginalName` scrub class | [metadata plan](../../apps/filesquid/crates/metadata/resources/plan.md) | FS-1.3 | 4 |
| API-F2.13 Command envelopes and operation-ID idempotency on the file write routes | [api plan](../../apps/filesquid/crates/api/resources/plan.md) | FS-2.5 | 10 |
| IO-1.6 Copy-guard warnings name the object | [ocfl plan](../../apps/filesquid/crates/ocfl/resources/plan.md) | FS-2.9 | 2 |
| API-F2.12 Operation IDs on file-route commits | [api plan](../../apps/filesquid/crates/api/resources/plan.md) | FS-2.9 | 4 |
| CLI-0.2 Journal every `fsquid` command | [cli plan](../../apps/filesquid/crates/cli/resources/plan.md) | FS-2.9 | 6 |

API-F1.4 grows from 14 to 17 hours and API-F1.13 shrinks from 6 to 3 hours (the stored name moved). The MVP estimate grows by 26 hours, from 968 to 994: FS-1.3 62 to 66, FS-2.5 104 to 114, FS-2.9 58 to 70. All five rows sit in MVP global tasks, so no milestone changes.

## Files changed

- `resources/architecture/036-single-fsquid-cli.md`: dated amendment (Status, Context, Decision 8, Rationale, Assumptions, Alternatives, Consequences, Related); the nine ISO 42010 items are kept.
- `resources/architecture/038-import-file-record.md`: dated amendment (Status, Decision 6, new Decisions 8 to 10, Rationale, Alternatives, Consequences, Related); the nine items are kept.
- `apps/filesquid/crates/api/resources/srs.md`: API-S-002 (envelopes on every mutation route), API-S-026 (the record is the metadata module's file record; uploads create the per-file object), API-S-028 (file-route commits carry the operation message), API-S-031 (CLI data-directory rule, copy-guard object reference).
- `apps/filesquid/crates/metadata/resources/srs.md`: MD-S-008 (import-commit exception, `schemaorg:` key), MD-S-012 (prefix stays `schema:`), MD-S-013 (identity classes with `OriginalName`; parent F-046 added).
- `apps/filesquid/crates/ocfl/resources/srs.md`: new row OCFL-S-025 (the refused copy's warning names the object).
- `resources/srs.md`: SRS-F-046 (original name inside a shared record), SRS-F-061 (CLI journal location and refusal).
- `resources/prd.md`: F-046 (the **Use original filename** control governs the name inside a shared record), F-061 (CLI journal location and refusal).
- `apps/filesquid/crates/api/resources/plan.md`: API-F1.4, API-F1.13 and API-F2.10 rows updated; new API-F2.12 and API-F2.13 rows.
- `apps/filesquid/crates/metadata/resources/plan.md`: new MD-0.10 row; summary path.
- `apps/filesquid/crates/ocfl/resources/plan.md`: new phase section and IO-1.6 row; total 161 to 163 hours.
- `apps/filesquid/crates/cli/resources/plan.md`: new phase section and CLI-0.2 row; intro names the two new dependencies.
- `resources/implementation-plan.md`: Descriptions, Execute lists, Estimated Time and Agent Prompt order of Tasks FS-1.3, FS-2.5 and FS-2.9; the module dispatch register; the Summary Timeline (phase F1 164 to 168, phase F2 486 to 508, total 2,821 to 2,847, MVP 968 to 994).
- `resources/implementation-plan-checklist.md` and `resources/assets/mvp-progress.svg`: the five new module rows, then `python3 scripts/docs/update_mvp_progress.py`.
- `README.md`: `## View logs` gains a bullet that marks the CLI journal as planned (Task CLI-0.2), not yet working; no command is documented for it.
- `resources/tasks/Task-docs-owner-decisions-2026-10-20-Walkthrough.md`: this file.

The README's `# About this project` section was reviewed after the PRD edits; neither edit changes the product vision it summarises, so it is unchanged. No BDD feature file was added: the scenarios for the new behaviour belong to the implementing tasks, whose Agent Prompts ask for them, and a scenario without step definitions would fail the crates' cucumber runners.

## Sync Execution Report

- **ADR → SRS:** ADR 038 Decisions 6, 8, 9 and 10 cascaded into API-S-026, MD-S-008, MD-S-012 and MD-S-013; ADR 036 Decision 8 into API-S-031. Items 4 and 5, which change no ADR, were written into OCFL-S-025, API-S-028, API-S-031 and API-S-002.
- **SRS → PRD:** SRS-F-046 and SRS-F-061 were updated, and the PRD sections that state the affected behaviour (F-046 outgoing names, F-061 complete coverage) now say it. PRD and SRS anchors are unchanged, so the traceability links stay valid; no task ID or timeline was written into the PRD or an SRS.
- **Plans:** the api, metadata, ocfl and cli module plans, the implementation plan (descriptions, dispatch register, hours) and the checklist with its regenerated progress bar.
- **Not changed:** ADR 033 and `logging-enforce.md` (no new journal exception), ADR 027 and ADR 030 (already consistent with the decisions), UI-S-012 (it already states the convention that item 5 confirms).

## Cubic round 1

Cubic's first review (head `74a1606`) found 13 issues (0 P1, 9 P2, 4 P3), all fixed in the round-1 commit: API-F2.12 now takes its operation ID from API-F2.13's envelopes and depends on it (and Task FS-2.9 on Task FS-2.5); API-F1.4 states the verified upload size and the filtered stored extension; ADR 036, API-S-031 and CLI-0.2 define the `cli.route` entry, the `journal_unavailable` refusal line and the `journal_write_failed` line; MD-0.10 defines `FileRecord::scrubbed`; the CLI plan's intro says the two dependencies are planned; the copy-guard rule is its own row, OCFL-S-025; the README qualifies the 100% claim; this walkthrough states the extension guard and the TDD-ENFORCE note.

## Verification

The module SRS and plan files live inside crates, so `bash scripts/changed_scope.sh` classifies this documentation-only change as `rust:filesquid-api,filesquid-cli,filesquid-desktop,filesquid-metadata,filesquid-native,filesquid-ocfl,filesquid-prov,filesquid-server` (changed: crates/api, crates/cli, crates/metadata, crates/ocfl), not `docs`. The rules therefore require the full suite, `TMPDIR=/dev/shm/owner-decisions FULL=1 bash scripts/pre-commit.sh`, run after `npm ci` in `apps/filesquid/ui` and `apps/imagesquid`.

The first full run (cold Cargo cache, 887s) checked the working tree before the first commit. Three small documentation edits (an ADR 036 sentence, the API-F2.13 upload text and one walkthrough row) were made while it ran; the runs in the table below cover them. Its raw summary lines:

```text
✅ Branch Validation passed.
✅ Traceability Validation passed: 96 PRD requirements, 84 resolved SRS references.
✅ Workspace Boundary validation passed.
✅ Format validation passed.
✅ ADR structure (ISO 42010) passed.
✅ PRD allocation passed.
✅ Plan traceability passed.
✅ Relative links and anchors passed.
✅ MVP progress bar passed.
✅ Task cleanup passed.
✅ Documentation validation passed.
✅ Change scope validation passed (168 assertions).
ℹ️  Test scope: full
✅ Clippy passed (108s).
✅ Workspace tests passed (264s).
✅ Rustdoc passed (48s).
✅ Vitest passed (3s).
✅ Lint and test validation passed (test scope: full).
✅ All Pre-Commit Hooks Passed! Test scope: full (887s)
```

Each pushed commit was then checked on its own tree before it was pushed (separate runs, warm cache):

| Commit | Run | Summary line |
|---|---|---|
| `74a1606` (first PR head) | `FULL=1 bash scripts/pre-commit.sh` | `✅ All Pre-Commit Hooks Passed! Test scope: full (318s)` |
| `528903c` (Cubic round 1 fixes) | `bash scripts/pre-commit.sh` | `✅ All Pre-Commit Hooks Passed! Test scope: rust:filesquid-api,filesquid-cli,filesquid-desktop,filesquid-metadata,filesquid-native,filesquid-ocfl,filesquid-prov,filesquid-server (343s)` |
| `8de2781` (Cubic round 2 fix) | `FULL=1 bash scripts/pre-commit.sh` | `✅ All Pre-Commit Hooks Passed! Test scope: full (316s)` |

A commit that changes this table cannot list its own run; that run's summary line is posted in the pull request.
