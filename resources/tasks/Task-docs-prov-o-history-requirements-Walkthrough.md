# Task docs/prov-o-history-requirements: Walkthrough

Requirements, architecture and plan for the owner-approved task "PROV-O history", the C2PA scoping clean-up, and the owner decisions of 2026-10-19 on stored names, display names, the per-file metadata record, the durable activity journal, the Journal tab and the file panel tabs. No product code changed. One gate script changed: `scripts/docs/check_plan_traceability.py` now recognises `PROV-S` rows (two regular expressions extended, so it checks more, never less); the pull request's change scope is `full`, so the full hook ran.

TDD-ENFORCE: Not applicable - requirements, ADRs, plans and BDD scenarios without step definitions; the one-line checker extension is exercised by the documentation gate on this branch, which now checks the seven `PROV-S` rows.

## Summary of changes

- **New ADR:** [ADR 040](../architecture/040-prov-o-history.md), PROV-O history of every OCFL version, independent of C2PA, from the stand-alone `filesquid-prov` module with an optional `ocfl` feature; records written in the same OCFL commit at `provenance/<version>.jsonld` (profile `filesquid-prov/1`, embedded context, PROV namespace `http://www.w3.org/ns/prov#`); the owner agent is the installation UUID; one-time reconstruction for earlier versions; prerequisites for journal linkage. It amends ADR 015, ADR 026 and Decision 6 of ADR 037.
- **Amended ADRs:** [ADR 033](../architecture/033-activity-journal-and-live-health-monitoring.md) (Decision 7: durable journal, coverage gaps, Journal tab, export, mapping table), [ADR 038](../architecture/038-import-file-record.md) and [ADR 027](../architecture/027-filesquid-metadata-pluggable-stores.md) (one `<file-uuid>.metadata.md` per file holding the import record), plus dated notes in ADRs 015, 026 and 037 and the ADR index.
- **PRD:** new [F-067](../prd.md#f-067-prov-o-history-of-every-version-and-change) (Mandatory, MVP, no `c2pa` dependency); F-043 renamed "C2PA records and action disclosure", scoped to the `c2pa` module and the ImageSquid release, its PROV-O criterion moved to F-067; F-002 explains that every import is tracked and described without C2PA, and specifies stored names, display names and the metadata record; F-001, F-008, F-010, F-042, F-046, F-055, F-061, F-063 and the release and allocation tables updated.
- **SRS:** SRS-F-067 and new rows CORE-S-010, OCFL-S-024, MD-S-014, API-S-027 to API-S-032, DE-S-011 to DE-S-013, UI-S-019 to UI-S-022, and the new module SRS [`crates/prov`](../../apps/filesquid/crates/prov/resources/srs.md) (PROV-S-001 to PROV-S-007); MD-S-008 and MD-S-013 rewritten for the per-file metadata record.
- **Vision:** FE-082, U-89, T-058 added; U-78, U-88, FE-033, FE-036, C-011, C-033, C-056, T-052, the modules table and the module specifications updated.
- **Plan:** global [Task FS-2.8](../implementation-plan.md#task-fs-2.8) (PROV-O history, 76 hours: CO-0.11, IO-1.5, PV-0.1, MD-0.9, PV-0.2, API-F2.8, DE-0.10, API-F2.9) and [Task FS-2.9](../implementation-plan.md#task-fs-2.9) (durable journal, Journal tab and file panel tabs, 58 hours: API-F2.10, API-F2.11, CL-1.9, DE-0.11, UI-F2.15, UI-F2.16), both in phase F2 before the gate; Task FS-3.9 depends on both. API-F1.13 (display and download names) joins Task FS-1.3, MD-0.8 grows to 10 hours, PV-F3.1 joins Task FS-3.3. MVP total 968 hours. The checklist and the MVP progress bar were regenerated.
- **BDD scenarios:** [`prov_o_history_F-067.feature`](../../apps/filesquid/crates/api/tests/features/prov_o_history_F-067.feature) (API), [`prov_o_history_F-067.feature`](../../apps/filesquid/ui/tests/features/prov_o_history_F-067.feature) (file panel tabs), [`durable_journal_and_journal_tab_F-061.feature`](../../apps/filesquid/crates/api/tests/features/durable_journal_and_journal_tab_F-061.feature), and new scenarios in the F-002, F-010 and F-046 UI features. Step definitions arrive with the implementing tasks; the Cucumber harness skips unbound steps.
- **Checker:** `scripts/docs/check_plan_traceability.py` now also recognises `PROV-S` rows, so the new module SRS is checked like the others.

## Owner decisions recorded (2026-10-19)

1. PROV-O history is in the MVP, independent of the `c2pa` module (option B); C2PA is release 2 with ImageSquid, and the FileSquid requirements no longer describe automatic C2PA records on import as part of FileSquid's own import.
2. Record contents (A to K): one `prov:Activity` per committed operation and OCFL version; covered operations (import, rename, move, hide, delete, restore, purge, metadata edit, export, scrub, revert, repair, catalog organisation); caches, previews and no-op saves excluded; failed and refused attempts in the journal only; entities and relations; names may appear inside the owner's library and are redacted per F-046 when shared; operation ID, journal entry IDs and object reference on every activity; same-commit storage; one-time reconstruction that invents no agents; API, file panel and JSON-LD export; stand-alone `filesquid-prov` that never depends on `filesquid-native` or `filesquid-c2pa`.
3. Agent correction: no `prov:Person`. Each installation generates one UUID v4 on first run, stored with the host's data and the same across restarts; `urn:uuid:<installation-uuid>` is the owner agent. The FileSquid software agent and the reserved tool/model agent stay.
4. Stored names: originals are stored as `<file-uuid>.<ext>`; beside them `<file-uuid>.metadata.md` is THE metadata record (original file name, extension, format and MIME type, import facts, descriptive metadata). There is no separate `file-record.md`. Every surface and owner download uses the original name (or rename); sharing and publication keep the FileId default (C-033), and original names are recoverable with ordinary tools.
5. The activity journal is recoverable from disk: per-commit entries in the same OCFL commit, hourly (and on close) sealed JSON Lines segments in journal OCFL objects, rebuild from files restores the journal; at most one hour of entries that changed nothing may be lost. Coverage gaps (client events, engine warnings, library intents, CLI, metrics snapshots) are closed.
6. A Journal tab and a JSON Lines or CSV export; the file panel offers four tabs with one look and behaviour: File facts, Descriptive, PROV-O and Audit log (mockups referenced by file name in the separate mockups pull request).

## Sync Execution Report

| Layer | Documents updated |
|---|---|
| ADR → SRS | ADR 040 (new) and the ADR 027, 033 and 038 amendments cascaded into [srs.md](../srs.md) (SRS-F-001, F-002, F-008, F-010, F-043, F-046, F-055, F-061, F-062, F-065, new F-067, stack list, traceability table) and the module SRS files of core, ocfl, metadata, api, desktop, ui and the new prov module. |
| SRS → PRD | [prd.md](../prd.md): F-067 added; F-001, F-002, F-008, F-010, F-011, F-014, F-027, F-041, F-042, F-043, F-046, F-055, F-061, F-062 copy rule, F-063, TC-005, Technology Stack, release and delivery tables, RC-009, RC-010 and the vision allocation updated. |
| Vision | [product-vision.md](../product-vision.md): FE-082, U-89, T-058 added; C2PA timing and naming statements corrected. |
| README | Root `README.md` "About this project": PROV-O history bullet and the C2PA bullet scoped to the second consumer release. |
| Architecture narrative | [architecture.md](../architecture.md): C2PA timing statements scoped to the `c2pa` module; PROV-O history added to the provenance profile and the evidence gates. |
| Plans | [implementation-plan.md](../implementation-plan.md), [implementation-plan-checklist.md](../implementation-plan-checklist.md) and the module plans of core, ocfl, metadata, api, desktop, ui and the new prov module. |
| BDD | The feature files listed above. |

C2PA timing cross-check: ADR 035, ADR 037, C-025 and C-056 agree that C2PA (read, validation, records, routes, panel) ships with the ImageSquid release in phase C; no PRD or vision statement now places automatic C2PA records in the MVP or release 1. The ImageSquid PRD keeps its own F-043 wording, which belongs to the release that ships the module.

## Open questions for the owner

- Whether a single-file download, not only an export or a ZIP download, records a PROV-O export activity (each would add a version to the library's history object).
- How individual `filesquid-server` users are attributed in PROV-O once accounts exist (Task FS-3.10), given that the owner agent is the installation UUID.
- `AGENTS.md` section 15 and `.agents/rules/src-isolation.md` name only the metadata module's optional `ocfl` store as an optional cross-module dependency; the `prov` module's optional `ocfl` feature is not listed there yet (owner-maintained files, not changed here).
- Client events: the route is bounded (20 events, 16 KiB, 60 requests a minute); events a client cannot deliver are reported as one `ui.console.overflow` count. Confirm that this count is acceptable under the no-summarising rule of `logging-enforce.md`.

## Validation

Raw output (step lines) of `FULL=1 bash scripts/pre-commit.sh` on commit `bb83c39`; the run on each later head commit is quoted in the pull request with its `Test scope: full` summary line.

```text
✅ Branch Validation passed.
Starting Port Isolation Validation...
✅ ./apps/filesquid/ui uniquely bound to 5173
✅ ./apps/filesquid/ui uniquely bound to 4173
✅ ./apps/imagesquid uniquely bound to 5174
✅ ./apps/imagesquid uniquely bound to 4174
✅ filesquid-server uniquely bound to 8083
Starting Document Sync Validation...
Starting PRD/SRS Traceability Validation...
✅ Traceability Validation passed: 96 PRD requirements, 84 resolved SRS references.
Starting Portable Documentation Validation...
Starting Workspace Boundary Validation...
✅ Workspace Boundary validation passed.
Starting logging rule validation...
✅ Logging rule validation passed (no relevant staged files).
Starting Format Validation...
✅ ./apps/filesquid is properly formatted.
✅ ./apps/filesquid/ui is properly formatted.
✅ ./apps/imagesquid is properly formatted.
✅ Format validation passed.
Starting Documentation Validation...
✅ ADR structure (ISO 42010) passed.
✅ PRD allocation passed.
✅ Plan traceability passed.
✅ Relative links and anchors passed.
✅ FileSquid and ImageSquid document boundary passed.
✅ MVP queue and merge driver tests passed.
✅ MVP progress bar passed.
✅ Task cleanup passed.
✅ Documentation validation passed.
Starting Change Scope Validation...
✅ Change scope validation passed (166 assertions).
Starting Lint and Test Validation...
✅ Clippy passed (1s).
✅ Workspace tests passed (55s).
✅ Module crate alone: filesquid-core passed (2s).
✅ Module crate alone: filesquid-ocfl passed (28s).
✅ Module crate alone: filesquid-metadata passed (10s).
✅ Module crate alone: filesquid-c2pa passed (79s).
✅ Module crate alone: filesquid-api passed (11s).
✅ Module crate alone: filesquid-native passed (30s).
✅ Clippy (filesquid-metadata, all features) passed (0s).
✅ Tests (filesquid-metadata, all features) passed (9s).
✅ wasm32 build (filesquid-metadata, default features off) passed (10s).
✅ wasm32 build (filesquid-metadata, ocfl feature) passed (0s).
✅ Rustdoc passed (5s).
✅ Clippy (desktop, test-instrumented) passed (5s).
✅ Desktop tests (test-instrumented) passed (20s).
✅ ESLint passed (5s).
✅ TypeScript passed (5s).
 Test Files  33 passed (33)
      Tests  600 passed (600)
✅ Vitest passed (17s).
✅ ESLint passed (1s).
✅ TypeScript passed (4s).
 Test Files  4 passed (4)
      Tests  37 passed (37)
✅ Vitest passed (3s).
✅ Lint and test validation passed (test scope: full).
✅ All Pre-Commit Hooks Passed! Test scope: full (333s)
exit=0
✅ All Pre-Commit Hooks Passed! Test scope: full (333s)
```
