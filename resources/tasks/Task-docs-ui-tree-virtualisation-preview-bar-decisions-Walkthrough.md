# Walkthrough: record the owner decisions of 2026-10-30 on the UI-F3.1 results

Branch: `docs/ui-tree-virtualisation-preview-bar-decisions`. Documentation and plan changes, plus the comments of the performance suite's declared limits and one unit test that keeps them in step with UI-S-035. No code under `apps/filesquid/ui/src` or `apps/filesquid/crates` changed. TDD-ENFORCE: applied to the one test change (red against the old UI-S-035, green against the new); not applicable to the documentation.

## The decision

The owner, verbatim, 2026-10-30, answering the orchestrator's four recommendations on the [UI-F3.1 results](../../apps/filesquid/ui/resources/tasks/Task-UI-F3.1-Walkthrough.md):

> "Okay implement all these recommendations"

| Item | Recommendation (now the decision) | Where it is recorded |
|---|---|---|
| 1 | Virtualise the navigation tree: the DOM holds the window plus two overscans of 5, as the list does; keep the tree semantics, the saved collapse state and pins; remove the declared known gap (`treeGap`); Large open-folder and back result p95 ≤ 100 ms; the packaged macOS measurement (DE-F3.2) gives the final answer | [UI-S-035](../../apps/filesquid/ui/resources/srs.md) (DOM bound, decision text in place of "Known gap (owner decision needed)"); new Task UI-F3.4 in the [UI plan](../../apps/filesquid/ui/resources/plan.md); [Task FS-3.4](../implementation-plan.md#task-fs-3.4); DE-F3.2 in the [desktop plan](../../apps/filesquid/crates/desktop/resources/plan.md) |
| 2 | Confirm the limits of UI-S-035 (warm N = 120 after a warm-up of 20; 20 cold loads per profile; p99 ≤ 200 ms per 100 ms class; 5 s sample timeout; missed frame above 25.05 ms; at most 5% missed; frame p99 ≤ 50 ms; stalls above 100 ms at most 0.1% of frames; failure state within 2,000 ms); the two 12-megapixel original limits (p95 ≤ 2,000 ms warm, ≤ 3,000 ms cold) stay provisional and DE-F3.2 revisits them | UI-S-035; `tests/performance/budgets.ts` (`PROVISIONAL_LIMITS`); `tests/unit/perf-budgets.test.ts`; DE-F3.2 Status and Agent Prompt; the performance feature file; root README |
| 3(a) | Build the preview's top bar (collection, badge, display name, "{position} of {total}", previous and next) and zoom chip ("Fit · {percent}%" with "Loading full size") in the MVP | new Task UI-F2.19 under [Task FS-2.5](../implementation-plan.md#task-fs-2.5), so [Task FS-3.9](../implementation-plan.md#task-fs-3.9) depends on it |
| 3(b) | Defer the filmstrip after the MVP | [UI-S-031](../../apps/filesquid/ui/resources/srs.md); a **Deferred:** note in UI-F2.19's Status (the UI plan's convention for deferred work with no task yet); Task FS-2.5 and Task FS-3.9 (deferred scope); the [v3 mockup index](../ui-mockups/v3/README.md) |
| 3(c) | Media frame radius: the 8 px `--fs-radius` token, not a one-off 10 px | Mockups unchanged: see the finding below; UI-F2.19 keeps the built 8 px |
| 4 | Keep the 10,000-node cap per level for the MVP; pins work by handle; paging more of a level on scroll after virtualisation is a post-MVP option, not scheduled | UI-S-035 and [UI-S-017](../../apps/filesquid/ui/resources/srs.md) (the requirement that states the tree's limit); UI-F3.1 and UI-F3.4 Status |

## Mockup 22: what the media frame draws

The media frame of [`22-preview-image.svg`](../ui-mockups/v3/22-preview-image.svg) and [`22-preview-image-dark.svg`](../ui-mockups/v3/22-preview-image-dark.svg) is the 640 × 426 px rectangle at (360, 178), line 79 of each file: `<rect x="360" y="178" width="640" height="426" rx="4" fill="#BFD7E6"/>`. It draws a 4 px corner, not 10 px. The landscape paths drawn over it end in square corners (`V604H360z`), so the frame shows rounded top corners and square bottom corners. Neither file has any `rx="10"` or `rx="8"`, and no clip path. Since the mockups do not draw 10 px, the SVGs are unchanged; the 10 px in the UI-F3.1 Walkthrough ("Gaps recorded") and in the comment of `tests/performance/80-composition.perf.spec.ts` does not match the mockup.

## New tasks

| Task | Plan | Global task | Estimate |
|---|---|---|---|
| UI-F2.19 Preview top bar and zoom chip | [UI plan](../../apps/filesquid/ui/resources/plan.md), phase F2 | FS-2.5 (MVP) | 6 hours, proposed, not yet confirmed by the owner |
| UI-F3.4 Virtualised navigation tree | [UI plan](../../apps/filesquid/ui/resources/plan.md), phase F3 | FS-3.4 (MVP), before DE-F3.2 | 16 hours, proposed, not yet confirmed by the owner |

UI-F3.4 goes in Task FS-3.4 rather than FS-2.5: the gap is a budget of the performance gate, the suite that declares it is UI-F3.1's, and DE-F3.2 in the same global task must measure the virtualised tree. Task FS-3.9 depends on FS-3.4, so the row lands before the MVP gate. The MVP estimate grows from about 1,170 to about 1,192 hours.

## Files changed

- `apps/filesquid/ui/resources/srs.md`: UI-S-017, UI-S-031, UI-S-035.
- `apps/filesquid/ui/resources/plan.md`: new UI-F2.19 and UI-F3.4; UI-F3.1 Status.
- `apps/filesquid/crates/desktop/resources/plan.md`: DE-F3.2 Status, Dependencies, Related Requirements and Agent Prompt.
- `resources/implementation-plan.md`: Tasks FS-2.5, FS-3.4 and FS-3.9; the module dispatch register; the summary timeline.
- `resources/implementation-plan-checklist.md` and `resources/assets/mvp-progress.svg`: the two new rows, then `python3 scripts/docs/update_mvp_progress.py`.
- `apps/filesquid/ui/tests/features/`: new `virtualised_navigation_tree_UI-S-035.feature` and `preview_top_bar_and_zoom_chip_UI-S-031.feature` (documentation for the UI rows, which write the tests behind them); comments in `production_build_browsing_performance_NFR-002.feature`.
- `apps/filesquid/ui/tests/performance/budgets.ts`, `apps/filesquid/ui/tests/unit/perf-budgets.test.ts`.
- `README.md` (the performance paragraph) and `resources/ui-mockups/v3/README.md` (the row of mockup 22).

## Sync Execution Report

- **SRS:** UI-S-017, UI-S-031 and UI-S-035 of the UI module SRS. The product SRS and the PRD need no change: NFR-002, NFR-003 and RC-002 already ask for the limits, and no PRD requirement names the filmstrip or the tree cap.
- **Plans:** the implementation plan, the UI and desktop module plans and the checklist.
- **Mockups:** only the v3 index row of mockup 22; no SVG changed.
- **ADRs:** none needed. A search of `resources/architecture/` for virtualisation, the navigation tree, the filmstrip, the radius and the 12-megapixel limit found ADR 031 (virtualisation built by hand as custom elements) and ADR 007 (a bounded visible window and the p95 budgets), which these decisions follow.

## Verification

The new unit test, red against the UI-S-035 on `origin/main`, then green:

```text
     × UI-S-035 records the limits as confirmed, not proposed 18ms
     × exactly the two 12-megapixel original limits are provisional, in code and in UI-S-035 8ms
      Tests  2 failed | 18 passed (20)
```

After Cubic round 1 the test also checks every confirmed limit against the text of UI-S-035 (21 tests):

```text
      Tests  21 passed (21)
```

`bash scripts/verify_docs.sh` (after Cubic round 1):

```text
✅ ADR structure (ISO 42010) passed.
OK: every vision ID is allocated
✅ PRD allocation passed.
OK: every requirement is accounted for in a plan task
✅ Plan traceability passed.
checked 309 Markdown files: 0 broken link(s)
✅ Relative links and anchors passed.
checked 177 documents: 0 link(s) cross the FileSquid/ImageSquid boundary
✅ FileSquid and ImageSquid document boundary passed.
Ran 54 tests in 5.661s
OK
✅ MVP queue and merge driver tests passed.
✅ MVP progress bar passed.
✅ Task cleanup passed.
✅ Documentation validation passed.
```

The change scope against `origin/main` is `rust:filesquid-desktop npm:apps/filesquid/ui`: the desktop plan file belongs to its crate, so the full hook runs. `FULL=1 bash scripts/pre-commit.sh` on the first commit (`748283f1`):

```text
ℹ️  Test scope: full
✅ Clippy passed (143s).
✅ Workspace tests passed (310s).
✅ Vitest passed (45s).
✅ All Pre-Commit Hooks Passed! Test scope: full (1051s)
```

The browser performance suite (`npm run test:perf`) was not run: no code it measures changed. Later commits' hook lines are in the pull request.

## Cubic round 1

Eleven findings (seven P2, four P3). Fixed: dark-theme outlines for the zoom chip and loading scenarios; label-table assertions limited to static wording; README and mockup index wording (UI-F3.4 not yet done; mockup 22 still draws the filmstrip); the confirmed limits checked against UI-S-035; the profile sentence of UI-S-035 (the API pages 1 to 1,000 nodes; the UI's own cap is 50 pages of 200); one tab stop per section's tree; DE-F3.2's Related Tests; the link-check count; this section's hook output; and UI-S-035's staging words, which now describe the state ("while the tree draws every node of a level") and keep the task reference the decision asked for.
