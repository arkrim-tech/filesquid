# Task 0.8 Plan: Test reporting, capability register, and foundation evidence (global, documentation remainder)

Branch `docs/task-0.8-foundation-evidence`. Owner decision 2026-10-23 (decision 6): the remaining work of global [Task 0.8](../implementation-plan.md#task-0.8) is documentation only. The module slice [UI-0.6](../../apps/filesquid/ui/resources/plan.md) (Playwright production suite and Allure) is done and ticked ([walkthrough](../../apps/filesquid/ui/resources/tasks/Task-UI-0.6-Walkthrough.md)); the server half (BE-0.6) moved to Task FS-3.10 ([ADR 035](../architecture/035-macos-first-desktop-mvp.md)). No Rust, TypeScript, script or gate changes.

## Traceability

- PRD: [F-040](../prd.md#f-040-compatibility-and-parity-register), [NFR-002](../prd.md#nfr-002-benchmark-envelope-and-responsiveness), [NFR-003](../prd.md#nfr-003-bounded-processing-and-migration-endurance), [NFR-008](../prd.md#nfr-008-maintainability-and-verified-engineering), [RC-002](../prd.md#rc-002-performance-and-resource-bounds), [RC-008](../prd.md#rc-008-complete-parity-and-documentation-traceability), [TC-003](../prd.md#tc-003-platform-and-provider-limits), [TC-004](../prd.md#tc-004-decision-status-and-repository-alignment).
- SRS: [SRS-F-040](../srs.md#srs-f-040-compatibility-and-parity-register), [SRS-NFR-002](../srs.md#srs-nfr-002-benchmark-envelope-and-responsiveness), [SRS-NFR-003](../srs.md#srs-nfr-003-bounded-processing-and-migration-endurance), [SRS-NFR-008](../srs.md#srs-nfr-008-maintainability-and-verified-engineering), [SRS-TC-003](../srs.md#srs-tc-003-platform-and-provider-limits), [SRS-TC-004](../srs.md#srs-tc-004-decision-status-and-repository-alignment).
- ADRs: [ADR 017](../architecture/017-test-runners-and-reporting.md), [ADR 033](../architecture/033-activity-journal-and-live-health-monitoring.md), [ADR 007](../architecture/007-durable-jobs-and-interactive-performance.md).
- Gherkin: [`compatibility_and_parity_register_F-040.feature`](../../apps/filesquid/crates/server/tests/features/compatibility_and_parity_register_F-040.feature), [`capability_register_F-040.feature`](../../apps/filesquid/crates/core/tests/features/capability_register_F-040.feature), [`benchmark_envelope_and_responsiveness_NFR-002.feature`](../../apps/filesquid/crates/server/tests/features/benchmark_envelope_and_responsiveness_NFR-002.feature), [`maintainability_and_verified_engineering_NFR-008.feature`](../../apps/filesquid/crates/server/tests/features/maintainability_and_verified_engineering_NFR-008.feature). No scenario changes: the register and budgets are documents, and the existing scenarios already state the register rules that `filesquid-core` implements (CO-0.9).

## Files

| File | Change |
|---|---|
| `resources/capability-register.md` | New: the item-level F-040 capability register (Photos baseline, every F, NFR and TC ID, the drive-parity table, the no-star-ratings entry) |
| `resources/foundation-evidence.md` | New: test-reporting evidence, the proposed NFR-002/NFR-003/RC-002 budgets recorded as proposed, performance-profile declarations, the TC-004 architecture-versus-scaffold record |
| `README.md` | View logs: the ADR 033 pieces mapped to Tasks FS-2.4 and FS-3.6; the optional OTLP observability profile with a verified command; About: link to the register |
| `resources/implementation-plan.md` | Task 0.8 ticked; Status links the two documents and the walkthrough |
| `resources/implementation-plan-checklist.md`, `resources/assets/mvp-progress.svg` | Task 0.8 ticked; `python3 scripts/docs/update_mvp_progress.py` |
| `resources/tasks/Task-0.8-Walkthrough.md` | New; this Plan and the ToDos are deleted when it exists |

Existing-document search (adr-verification, search before denial): `capability register`, `parity register`, `acceptance budget` and `proposed budget` appear in the PRD, the SRS, `architecture.md` §5.2, `crates/core` (CO-0.9 types, CORE-S-009) and plan rows, but no register document or budget document exists under `resources/`, `docs/` or any `apps/*/resources/`. `scripts/docs/validate-adrs.mjs` already reads `resources/architecture/` (line 15); it is checked, not changed.

## Granular constraints extracted from the requirements

### F-040 / SRS-F-040 (register content)

1. Every register entry carries platform, format/capture prerequisite, equivalent outcome, requirement ID, test evidence and a status from: **equivalent, adapter equivalent, limited, preservation-only, unavailable**. This task's rows add the owner's three-way reading: supported, limited, unsupported.
2. A committed conditional capability declares a nonempty minimum supported format/model/platform set before its implementation gate; declaring all formats unsupported cannot satisfy a mandatory feature; an unmet outcome needs documented stakeholder acceptance as a remaining gap.
3. Broad family coverage never hides an unresolved child; a parity claim names the products and versions compared against; preservation-only never counts as editable parity.
4. Fixtures cover macOS/Windows/Linux webviews, major desktop and mobile browsers, cameras and codecs, ICC/HDR, input methods and languages; the register also covers F-041 recorder completeness, F-044 publisher coverage, F-045 model/task contracts, F-046 extraction / `dcterms:` / `schema:` / native-write / scrubbing profiles and F-047 mappings at its separate gate; required capabilities cannot pass with an empty supported set.
5. Unsupported states explain what is preserved, viewable, editable and exportable and offer safe alternatives; Apple services map to FileSquid or ImageSquid services or authorised partners.
6. Each supported performance profile declares device, browser/webview, media format and dimensions, cache/readiness and concurrency coverage under NFR-002/003 with actual evidence and unresolved limits; macOS desktop first, then `filesquid-server`; Linux/Windows desktop and the encrypted browser host stay mandatory at maturity.
7. Mature product addresses every photo-manager feature family (ImageSquid C-036) without promising identical Apple rendering (C-037); preservation stays possible when editing is unavailable (C-038); ImageSquid gate order per C-040.
8. **No star ratings** is an unavailable-by-decision entry (favorites and keywords are the classification model; imported `xmp:Rating` is preserved under F-046, not editable).
9. The **drive-parity** table lists each documented difference with its reason and owning F-ID: no version thinning, deletion or free-space cap (F-065); no per-folder end-to-end encryption and no administrator switch; no login-password-derived keys, no manual re-share after key changes, no master key (F-029, F-034); no operator-key encryption at rest and no administrator recovery key on self-run servers (F-057); thumbnails, previews, indexes and trash inside the encryption boundary; no default AI overview (F-063); no activity feed, AT Protocol publication only per explicit request (F-064); empty folders not persisted and copy creates a new file (F-062); no comment emoji or smart picker and no `sharedwith:external` (F-063, F-064); no document editors, calendar, reminders or federated mounts.
10. UI copy comes from the active label table; item type in data and APIs is **file**.
11. The photo-manager parity register (named products and versions) belongs to the ImageSquid distribution (C-036, C-037); this register records FileSquid capabilities and marks those rows pending.

### RC-008 (traceability)

12. Every granular Photos inventory item has an F-040 status and evidence; all F, NFR and TC IDs map to specifications and tests; F-047, F-060 and RC-014 (institutional) and F-048 (optional) keep explicit pending status; aggregate percentages never hide mandatory gaps; the PRD is a draft input to that verification, not evidence it passed.

### NFR-002 / SRS-NFR-002 (proposed budgets; never achieved)

13. Test profiles: Small 10,000 assets / 20,000 resources / 100 GB / 100 albums; Medium 100,000 / 200,000 / 1 TB / 5,000; Large 1,000,000 / 2,000,000 / 10 TB / 50,000 albums / 10,000,000 memberships; include duplicate bytes, pairs, Unicode, dense memberships, late sidecars, high decoded-pixel counts, varied formats; a 100 GB single resource and a 250 GB archive part through streaming.
14. Desktop reference machine 4-core / 16 GB / SSD: warm viewport feedback p95 ≤100 ms; indexed metadata search p95 ≤2 s; first useful local viewport ≤3 s on the Large catalog; ≥100 queries/interactions after declared warm-up; cold behaviour reported separately.
15. Mobile/browser profile 4 GB device/network: warm cached control feedback p95 ≤100 ms; completed browse/search p95 ≤2 s excluding unavailable-original downloads, which still appear in end-to-end reports.
16. Resident prepared thumbnail/viewer images and prepared next slides paint the correct image at p95 ≤100 ms; placeholder, stale image or spinner never passes; preparation coverage reported; input acknowledgement, first useful preview and final-quality readiness measured independently.
17. 60 Hz: 16.7 ms frame budget (proposed); per profile define missed-frame rate, slideshow transition lateness, p95/p99 sample sizes, clock boundaries, warm-up and cache contents, error limits; other refresh rates need their own budget; reports show stalls and p95/p99, not averages alone.
18. Per feature before it ships: a nonempty format/device/local-or-remote matrix with useful-preview and full-quality deadlines for cold, warm, evicted and missing-derivative cases; the three gates pass independently; tests exercise idle and concurrent import/migration, thumbnail generation, indexing, provenance scans, batch metadata work, navigation reversals and jumps, cache loss and resource pressure.
19. Budgets remain **proposed until measured** on packaged `filesquid-desktop` and `filesquid-server` builds and the browser host; no FileSquid U-11 claim.

### NFR-003 / SRS-NFR-003 (proposed bounds)

20. Desktop staging cap 10 GiB; reserve max(2 GiB, 5 % of destination capacity); preflight chooses a larger cap or segmented path when a resource cannot fit; streamed large resources need no full extra copy.
21. Default active transfers ≤4; CPU-heavy workers ≤2; every queue has explicit finite limits and backpressure.
22. At most 5 total attempts per resource (initial included) before quarantine; exponential backoff capped at 60 s plus jitter; manual retry starts a new recorded recovery cycle under the same cap.
23. Migration process memory ≤2 GiB excluding separately budgeted ML models; restart reaches a resumable state within 30 s on the reference desktop without rescanning completed media; repeated bytes per route measured.
24. 72-hour mixed/fault soak shows no unbounded growth; post-warm-up non-cache memory growth ≤10 %; throughput reported as verified bytes/time with hardware, network, small-file mix and sidecar reconstruction time; no WAN-speed promise.
25. Heavy work outside the interactive path through bounded CPU/GPU/I/O/transfer queues; visible media first; deduplicated equivalent work; speculative work reduced under memory, disk, network, battery or thermal pressure; each scheduler profile defines a background-service guarantee or visible pause policy before implementation.
26. Per device/browser profile: finite memory/disk cache budgets and prefetch item/byte/time limits; cache keys on identity, content/recipe/metadata revision, view/sort/filter and permission scope; stale work cancelled; eviction never deletes originals, history or pinned offline media; peak decoded memory, occupancy, queues, hit/miss/eviction/redecode rates, speculative bytes and background progress measured.
27. Durable completed-item checkpoints for import/upload, thumbnail/index, provenance-scan and metadata batches; restart revalidates source, version and authorisation; no entire-batch restart, duplicate events or false-complete media.

### RC-002 (verification, proposed)

28. p95/p99 interaction and image readiness, frame stalls, missed-frame rate, slide lateness and preparation coverage in cold, warm, evicted and missing-cache states under idle and concurrent work; peak decoded/cache/process memory, staging, queue caps, cache rates, resume time, repeated bytes, throughput and the 72-hour soak recorded; cloud pilots additionally load-test a proposed **100 concurrently active sessions with 20 concurrent resumable transfers**, a proposed pilot test and not a capacity promise; absent limits or an all-unsupported difficult-media profile do not pass.

### NFR-008 / ADR 017 (test reporting)

29. Lint and format precede unit, integration, BDD, built-production UI and load tests; structured reports (Allure/JUnit) under each app's `tests/allure-results/`, cleared before each run; raw task walkthrough evidence; functions ≤60 lines; bounded loops; explicit cleanup and error paths; tests track changed requirements and isolate artifacts in each workspace's `tests/` directory.

### TC-003 / TC-004 (register and record)

30. PhotoKit permissions and library coverage, Google export/API restrictions, Microsoft placeholders and range semantics, browser lifetime and filesystem constraints, media licensing and capture dependencies are verified per supported version; Apple proprietary history, rendering and undocumented APIs are never assumed.
31. Stack candidates become reviewed ADRs before implementation; ADRs 001–016 and 022–028 as amended by 029–032 are the direction; the repository supplies scaffolded crates, hosts and UI shells, not the product features; the hypothetical architecture is labelled; accepted stack direction is not evidence that NFR-002/003 or U-11 were measured; named cloud region, payment, recorder implementation, job SQL, SPA-asset signing and independent crypto audit remain later work (T-034); Git is not the OCFL library; SHA-256 is the OCFL digest.

### ADR 033 (README section)

32. The README documents the activity journal (where it lives, what is journaled, retention and export), the Monitor dashboard (route `#/monitor`, desktop per-launch token, server admin token) and the optional OTLP export (off by default, store-and-forward, the Compose `observability` profile with VictoriaLogs and VictoriaMetrics, never bundled into the desktop app), tied to Tasks FS-2.4 (done) and FS-3.6 (open); every command shown is executed first ([command-verification.md](../../.agents/rules/command-verification.md)).

## Verification plan

- `node scripts/docs/validate-adrs.mjs` (confirms the ADR directory it reads).
- `docker compose --profile observability config --services` (the one profile command that runs without a Docker daemon in this environment; the daemon is unavailable here, so starting the profile stays with Task FS-3.6).
- `python3 scripts/docs/update_mvp_progress.py` and `--check`.
- `bash scripts/pre-commit.sh` on the head commit, summary line `Test scope: docs`, pasted raw into the walkthrough.
- Cubic review loop until no findings remain; no merge (the orchestrator decides).
