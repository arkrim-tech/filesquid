# Task UI-F3.4 Plan: Virtualised navigation tree

[Task UI-F3.4](../plan.md) is a UI module row of global [Task FS-3.4](../../../../../resources/implementation-plan.md#task-fs-3.4) (MVP performance). It makes `<fs-nav-tree>` draw only its window, which removes the known gap that Task UI-F3.1 recorded ([Finding 4](./Task-UI-F3.1-Walkthrough.md)).

Requirements: [UI-S-035](../srs.md), [UI-S-017](../srs.md), [UI-S-016](../srs.md), [UI-S-025](../srs.md), [UI-S-024](../srs.md), [UI-S-005](../srs.md), [UI-S-013](../srs.md); PRD [NFR-002](../../../../../resources/prd.md#nfr-002-benchmark-envelope-and-responsiveness), [NFR-003](../../../../../resources/prd.md#nfr-003-bounded-processing-and-migration-endurance), [RC-002](../../../../../resources/prd.md#rc-002-performance-and-resource-bounds), [F-004](../../../../../resources/prd.md#f-004-collections-folders-and-navigation), [NFR-006](../../../../../resources/prd.md#nfr-006-accessibility-and-usability).

Gherkin: [`virtualised_navigation_tree_UI-S-035.feature`](../../tests/features/virtualised_navigation_tree_UI-S-035.feature), [`production_build_browsing_performance_NFR-002.feature`](../../tests/features/production_build_browsing_performance_NFR-002.feature), [`navigation_preferences_UI-S-017.feature`](../../tests/features/navigation_preferences_UI-S-017.feature), [`accessibility_contracts_NFR-006.feature`](../../tests/features/accessibility_contracts_NFR-006.feature).

Mockups (active suite v3): [`55-navigation-tree-built.svg`](../../../../../resources/ui-mockups/v3/55-navigation-tree-built.svg), [`05-all-files-list.svg`](../../../../../resources/ui-mockups/v3/05-all-files-list.svg).

## Owner decisions (binding, verbatim)

- 2026-10-30: "Okay implement all these recommendations". Recommendation 1: virtualise the navigation tree so it holds only its window plus overscan, keeping its tree semantics, collapse state and pins. Recommendation 4: keep its cap of 10,000 nodes a level for the MVP.
- 2026-10-31: "accept recommendations". A capped level also shows a short limit line on screen.

## Constraints, copied from the requirements and the plan row

Each one has a gate with the same number in the ToDos.

1. **DOM bound.** The tree holds in the DOM at most the tree items in its window plus two overscans of 5, however many nodes its open levels hold (UI-S-035).
2. **Scrolling area.** The tree's scrolling area is never taller than the window (the sidebar); its content may be taller and scrolls inside it (UI-S-035, UI-S-025).
3. **Level cap.** A level shows at most its first 10,000 nodes: 50 pages of 200 (UI-S-017). Paging more of a level on scroll is not part of this task.
4. **Console message.** A level that stops at the cap says so on the console (UI-S-017).
5. **Limit line.** A capped level shows a short line on screen under the last node it shows, from the label key `navigationLevelLimit`, wording "Showing the first {count} only. Use search to find the rest.", in the default and the ImageSquid label tables (UI-S-017, UI-S-005). With the cap the line reads "Showing the first 10,000 only. Use search to find the rest."
6. **Roles.** The tree role on each section's tree and the tree item role on each item (UI-S-016, UI-S-035).
7. **`aria-level`** on every item (UI-S-035).
8. **`aria-setsize` and `aria-posinset`** counted over the whole level, not over the drawn items (UI-S-035). On the Large catalog a drawn collection has `aria-setsize="10000"` and its place in the level as `aria-posinset`.
9. **`aria-expanded`** on parents, **`aria-current`** on the current destination (plan row).
10. **One Tab stop** in each section's tree, no positive `tabindex` (UI-S-016).
11. **Keyboard.** Arrow keys, Home, End, expand and collapse reach every node of an open level, drawn or not; an undrawn node is brought into the window before it takes focus (UI-S-035, plan row).
12. **Collapse state.** At most 1,000 collapsed nodes, kept across a reload (UI-S-017).
13. **Pins.** At most 100 pinned destinations, kept across a reload; a pin beyond the cap opens by its handle; a pin the API answers `not_found` is shown as unavailable with a remove action (UI-S-017).
14. **Sections.** Sections by kind (Pinned, Collections, Virtual collections, Folders) with Recently deleted as the final row after a 1 px divider, as mockup 55 draws them (UI-S-017, UI-S-025).
15. **One tree element.** The virtual window replaces the full render inside `<fs-nav-tree>`; the data interface of UI-F2.1a (`NavigationSource`), the sections of UI-F2.8 and the item index of UI-F3.1 (`nav-tree-index.ts`) stay. No second tree (holistic-ui rule).
16. **Known gap removed.** `treeGap` leaves `tests/performance/known-gap.ts`, `20-browse.perf.spec.ts`, `70-load.perf.spec.ts`, `tests/unit/perf-known-gap.test.ts` and the re-export in `report.ts`; the KNOWN GAP comment leaves `production_build_browsing_performance_NFR-002.feature`; the README paragraph that records the gap changes with it.
17. **Result gate.** Large open-folder and back, result: p95 ≤ 100 ms and p99 ≤ 200 ms (UI-S-035).
18. **Feedback gate.** Large open-folder and back, feedback: p95 ≤ 100 ms and p99 ≤ 200 ms (UI-S-035).
19. **Under background events.** The same gates while 5 `navigation.change` events a second arrive (UI-S-035).
20. **Sample sizes.** 120 warm samples a class after a warm-up of 20 (at least 100 required); a sample that misses its predicate within 5,000 ms fails the run; 1440 × 900 at device scale factor 2; an idle control page misses at most 1% of its frames (UI-S-035). No budget, sample size or control is loosened.
21. **Row look.** Rows 30 px (1.875 rem) with 13 px text, 20 px of indent a level, a 16 px kind icon, the active tint with a 2.5 px brand bar, every colour and size through tokens, light and dark (UI-S-024, mockup 55).
22. **Accessibility contracts.** The shared stylesheet first, the 2 px `--fs-focus-ring` focus ring, logical properties only, motion from `--fs-motion-duration`, no text container fixed to a pixel height, no horizontal scroll at 320 px or 200% text (UI-S-016).
23. **Production artifact tests.** Playwright against `vite build` plus `vite preview` with strict DOM assertions: custom element defined, labels from the active label table, computed token values in light and dark, no console errors (UI-S-013).

## Approach

One element, the same data and the same sections. What changes is how a section's tree is drawn.

- **Rows, not nested lists** (`nav-tree-rows.ts`, pure). A section's open levels become one flat list of rows in document order. Each row knows its node, level, its place in its level and the size of that level, its parent row and whether it is open. A capped level adds a limit row after its last node.
- **Window** (`nav-tree-window.ts`, pure). From the band of a tree that is in view, the row height and the overscan of 5, it answers which rows to draw. The overscan is measured from the edges of the view, so the bound holds over all sections together.
- **Viewport** (`nav-tree-viewport.ts`, DOM reads). The tree does not scroll itself: the shell's sidebar scrolls it, and in the stacked layout the page does. The viewport reads the band of a tree that the window and every clipping ancestor leave in view, and follows scrolls and size changes of those ancestors. Tests inject their own.
- **Drawing** (`nav-tree-list.ts`). Each section's `ul[role=tree]` is as tall as all its rows (`--rows` times the row height) and holds only the rows in the window, each placed by `--r`, as the file list does. A row that has focus stays until focus leaves it.
- **Markup** (`nav-tree-view.ts`). A row is `li[role=none] > a[role=treeitem]` with `aria-level`, `aria-setsize`, `aria-posinset` and, for a parent, `aria-expanded`. The nested `ul[role=group]` and `aria-owns` go: a window cannot hold a group whose parent is not drawn, and the three attributes state the same structure. Rows are one line (30 px); a long name ends in an ellipsis and its full text is the row's `title`.
- **Keyboard** (`fs-nav-tree.ts`). The key rules of `nav-tree-keys.ts` are unchanged and now read the rows, not the DOM. A move to an undrawn row draws it first, then focuses it; the browser scrolls it into view and the window follows.
- **Tab stop.** The remembered item when it is drawn, else the current destination when it is drawn, else the first row in view, as the file list does.
- **Limit line.** `readLevel` reports that it stopped at the cap. The tree shows the line as a row of its own (two row heights, so the sentence fits the 240 px sidebar) and names it as the description of the level's last node, so a screen reader hears it there.
- **Cap by nodes.** `readLevel` also stops at 10,000 nodes when a source answers larger pages, so the cap is the one UI-S-017 states.

## Files

- New: `src/components/nav-tree-rows.ts`, `nav-tree-window.ts`, `nav-tree-viewport.ts`, `nav-tree-list.ts`.
- Changed: `src/components/fs-nav-tree.ts`, `nav-tree-view.ts`, `nav-tree-index.ts` (comment only if its interface holds), `src/navigation/loader.ts`, `src/navigation/model.ts`, `src/theme/labels.en.json`, `src/theme/labels.ts` if keys are typed there, `apps/imagesquid/src/theme/labels.en.json`.
- Tests, new: `tests/unit/nav-tree-rows.test.ts`, `tests/unit/nav-tree-window.test.ts`, `tests/ui/nav-tree-window.test.ts`, `tests/e2e/preview/navigation-tree-window.spec.ts`.
- Tests, changed: `tests/ui/nav-tree.test.ts`, `tests/unit/navigation-source.test.ts`, `tests/unit/perf-known-gap.test.ts`, `tests/performance/known-gap.ts`, `report.ts`, `20-browse.perf.spec.ts`, `40-resources.perf.spec.ts`, `70-load.perf.spec.ts`, the two feature files, and the server-backed `tests/e2e/navigation.spec.ts` where it asserts the nested markup.
- Docs: root `README.md` (the performance paragraph), `resources/srs.md` (UI-S-035's known-gap sentences), `resources/plan.md`, `resources/implementation-plan-checklist.md`, the MVP progress block.

## Verification

- Vitest: the new unit and UI tests fail before the code and pass after; the whole `npm test` passes.
- Playwright preview suite (`-c playwright.preview.config.ts`) under the machine's Playwright lock: the new spec and the specs that touch the tree.
- Performance suite (`npm run test:perf`), once near the end, under the lock: the Large open-folder and back records, idle and under events, with no gap declared. Before and after numbers go in the Walkthrough with the machine line.
- Not run on this machine: the server-backed suite (`playwright.config.ts`), which needs a `filesquid-server` build.
- `bash scripts/pre-commit.sh` with scope `npm:apps/filesquid/ui`.
