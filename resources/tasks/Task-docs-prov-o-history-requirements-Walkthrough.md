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

Decisions 1 to 6 are the state merged with this branch. Where the follow-up decisions 7 to 14 below change them (the file panel tabs and the Journal view of decision 6, the client-event count of decision 5), the follow-up wording is the authoritative UI and journal contract.

1. PROV-O history is in the MVP, independent of the `c2pa` module (option B); C2PA is release 2 with ImageSquid, and the FileSquid requirements no longer describe automatic C2PA records on import as part of FileSquid's own import.
2. Record contents (A to K): one `prov:Activity` per committed operation and OCFL version; covered operations (import, rename, move, hide, delete, restore, purge, metadata edit, export, scrub, revert, repair, catalog organisation); caches, previews and no-op saves excluded; failed and refused attempts in the journal only; entities and relations; names may appear inside the owner's library and are redacted per F-046 when shared; operation ID, journal entry IDs and object reference on every activity; same-commit storage; one-time reconstruction that invents no agents; API, file panel and JSON-LD export; stand-alone `filesquid-prov` that never depends on `filesquid-native` or `filesquid-c2pa`.
3. Agent correction: no `prov:Person`. Each installation generates one UUID v4 on first run, stored with the host's data and the same across restarts; `urn:uuid:<installation-uuid>` is the owner agent. The FileSquid software agent and the reserved tool/model agent stay.
4. Stored names: originals are stored as `<file-uuid>.<ext>`; beside them `<file-uuid>.metadata.md` is THE metadata record (original file name, extension, format and MIME type, import facts, descriptive metadata). There is no separate `file-record.md`. Every surface and owner download uses the original name (or rename); sharing and publication keep the FileId default (C-033), and original names are recoverable with ordinary tools.
5. The activity journal is recoverable from disk: per-commit entries in the same OCFL commit, hourly (and on close) sealed JSON Lines segments in journal OCFL objects, rebuild from files restores the journal; at most one hour of entries that changed nothing may be lost. Coverage gaps (client events, engine warnings, library intents, CLI, metrics snapshots) are closed.
6. (Superseded by decisions 11 and 12.) A Journal tab and a JSON Lines or CSV export; the file panel offers four tabs with one look and behaviour: File facts, Descriptive, PROV-O and Audit log (mockups referenced by file name in the separate mockups pull request).

### Owner answers to the open questions (2026-10-19, follow-up branch `docs/prov-o-owner-answers`)

After this branch was merged, the owner answered its four open questions. A follow-up branch applies them to the merged requirements; it changes `AGENTS.md` and `.agents/rules/`, so the owner merges it.

7. **Single-file downloads.** A single-file download does not create a PROV-O record; it is recorded in the activity journal only (`file.download`, opaque file ID and version). Exports, scrubbed exports, ZIP downloads and share or publication outputs produce new output and remain PROV-O activities in the per-library history object. Applied in [ADR 040](../architecture/040-prov-o-history.md) Decisions 1 and 5, [PRD F-010 and F-067](../prd.md#f-067-prov-o-history-of-every-version-and-change), [SRS-F-010 and SRS-F-067](../srs.md#srs-f-067-prov-o-history-of-every-version-and-change), API-S-028 ([API SRS](../../apps/filesquid/crates/api/resources/srs.md)), the API-F2.9 plan row ([API plan](../../apps/filesquid/crates/api/resources/plan.md)) and a new scenario in [`prov_o_history_F-067.feature`](../../apps/filesquid/crates/api/tests/features/prov_o_history_F-067.feature).
8. **Server accounts.** On `filesquid-server` (planned, Task FS-3.10) each account gets a UUID v4 and is its own `prov:Agent` (`urn:uuid:<account-uuid>`, no user name, e-mail address or display name), linked to the installation agent by `prov:actedOnBehalfOf`. Desktop records are unchanged: the installation UUID is the owner agent and there is no `prov:Person`. Recorded in ADR 040 Decision 4 as the planned approach, finalised in Task FS-3.10; PRD F-067 and SRS-F-067 state it; the [API-F1.9 plan row](../../apps/filesquid/crates/api/resources/plan.md) carries the dependency note (on API-F2.9) and [Task FS-3.10](../implementation-plan.md#task-fs-3.10) names it; a new API scenario covers it.
9. **`filesquid-prov`'s optional `ocfl` feature** is an allowed optional cross-module dependency under the same rule as the metadata module's `ocfl` store (never a default dependency; no dependency on `filesquid-native` or `filesquid-c2pa`): [AGENTS.md](../../AGENTS.md) section 15 and [src-isolation.md](../../.agents/rules/src-isolation.md) (Rust Exception Handling), with the matching statements in [architecture.md](../architecture.md), [ADR 029](../architecture/029-filesquid-unified-product-and-distributions.md) and ADR 040 Consequences.
10. **Client events.** The browser keeps a bounded in-page queue and retries with backoff, so the route's limits (20 events per request, 16 KiB, 60 requests a minute) only delay events and never merge them; every delivered event is its own journal entry. **Decided by the owner (2026-10-19): spill to IndexedDB.** The in-memory queue holds at most 10,000 events; when it is full, events spill into the page's IndexedDB store until its 50 MiB budget per origin is reached or a write is refused, after which the count below applies. At page close every event still queued is persisted there, and the next time FileSquid opens it delivers them, oldest first and each as its own journal entry, before new events; the route de-duplicates by client event ID. The one unavoidable gap, recorded in [logging-enforce.md](../../.agents/rules/logging-enforce.md), is storage that is unavailable or cleared: when IndexedDB is unavailable or refuses a write (quota or the budget), the client falls back to the in-memory queue and at page close sends one `ui.console.overflow` entry, as a keep-alive request, with the count of events it could not keep (a failed request loses the count uncounted, since there is no storage to persist it); events lost because the person cleared the site's data cannot be counted. Persisted events are bound to the host and principal that raised them and are never sent under another user's credential, and the route de-duplicates by client event ID, stored with each journal entry. Every other coverage gap of decision 5 stays closed. (This replaces the interim over-capacity count design.) Follow-up after Cubic's review of the merge with main: while storage accepts writes, each queued event is also written to IndexedDB as it is queued (write-through), so a crash or force-quit that skips page close loses no event whose write had committed; an event whose write had not committed is part of the same storage gap. Every event keeps its client event ID for life, and the overflow event carries `count` from 1 to 1,000,000. Sent batches stay queued until acknowledged, a `400` batch is split and an event refused on its own is replaced by a `client_event_refused` error event, and the overflow request is exempt from the rate bound. Applied in [ADR 033](../architecture/033-activity-journal-and-live-health-monitoring.md) Decision 7 and its mapping table, PRD F-061, SRS-F-061, API-S-030, the new UI-S-023 ([UI SRS](../../apps/filesquid/ui/resources/srs.md)), the API-F2.10, UI-F2.16 and FS-2.9 plan rows, and the client-event scenarios in [`durable_journal_and_journal_tab_F-061.feature`](../../apps/filesquid/crates/api/tests/features/durable_journal_and_journal_tab_F-061.feature). The earlier wording that reported every undeliverable event as one count is removed.

### Owner decisions on the file panel, the Logs view and the privacy statement (2026-10-19, same follow-up branch)

These match the mockups pull request #85.

11. **Three file panel tabs.** The file panel has three tabs with the same look, feel and behaviour: **File info** (formerly File facts: the `<file-uuid>.metadata.md` facts), **Description** (formerly Descriptive: Dublin Core and Schema.org) and **Provenance** (formerly PROV-O: the `prov:` attributes and values). There is no per-file Audit log tab; Versions still opens from the panel header button. Applied in PRD F-055, F-061 and F-067, SRS-F-055, SRS-F-061 and SRS-F-067, UI-S-002, UI-S-019, UI-S-020 and UI-S-022 ([UI SRS](../../apps/filesquid/ui/resources/srs.md)), [ADR 040](../architecture/040-prov-o-history.md) Decision 9, the UI-F2.15 plan row ([UI plan](../../apps/filesquid/ui/resources/plan.md)), the global plan, checklist and vision, and [`prov_o_history_F-067.feature`](../../apps/filesquid/ui/tests/features/prov_o_history_F-067.feature).
12. **Logs view.** The Journal view is renamed **Logs**, reached from a sidebar item placed directly below Settings (same icon). It is the single place to browse the audit log (all journal entries, filterable by time, library, operation, outcome and level, and file, object or operation ID), with export (JSON Lines or CSV). The Provenance tab links to its entries with **View in Logs**, which opens Logs filtered to that file or operation; Logs entries of committed operations link back to their Provenance activity; failed and refused entries are journal-only. Applied in PRD F-055 and F-061, SRS-F-061, UI-S-021, [ADR 033](../architecture/033-activity-journal-and-live-health-monitoring.md) Decision 7, ADR 040 Decision 9, the UI-F2.16 plan row and Task FS-2.9 (titles now say "Logs view"), and the UI and API feature files ([`durable_journal_and_journal_tab_F-061.feature`](../../apps/filesquid/crates/api/tests/features/durable_journal_and_journal_tab_F-061.feature) keeps its file name).
13. **Privacy statement (option 1a).** The [ADR 032](../architecture/032-hosting-and-privacy-model.md) Decision 6 statement stays in the product, unchanged in wording, but not as permanent header text: the header shows a compact host indicator (icon and short host label), and the full statement appears in its popover or tooltip and in the Privacy section of the Settings page. Applied in UI-S-004 and UI-S-018, a dated note in ADR 032, a follow-up note on the UI-F2.1b plan row (the pull request #62 header indicator becomes a compact badge with a popover) and a scenario in [`host_privacy_indicator_UI-S-018.feature`](../../apps/filesquid/ui/tests/features/host_privacy_indicator_UI-S-018.feature).

Mockups are cited by the final file names of pull request #85: `35-panel-file-info.svg`, `36-panel-provenance.svg`, `38-logs-view.svg` and `38-logs-view-dark.svg`, `39-logs-export.svg` and the new `40-host-indicator.svg`; `37-panel-audit-log.svg` no longer exists. The Monitor's "Activity journal" section keeps its name.

14. **No "Early access" label in the UI.** The About screen no longer says "early access"; it states that the macOS build is not signed or notarised by Apple yet (mockup `32-settings.svg`: "This macOS build is not signed or notarised by Apple yet."), and the DE-F3.1 packaged-window test checks that statement instead. The unsigned early-access DMG remains the release vehicle ([ADR 035](../architecture/035-macos-first-desktop-mvp.md)); only the UI label goes ([desktop plan](../../apps/filesquid/crates/desktop/resources/plan.md) DE-F3.1, a dated amendment of ADR 035 Decision 3, vision C-056 and the Task FS-3.9 evidence list).

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

None. The four questions this branch left open were answered by the owner on 2026-10-19 (decisions 7 to 10 above).

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
