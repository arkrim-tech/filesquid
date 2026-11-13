# Task UI-F3.4 ToDos: Virtualised navigation tree

Gates are numbered as the constraints in the [Plan](./Task-UI-F3.4-Plan.md).

## Before the code

- [x] Read AGENTS.md, every rule, the role file, UI-S-035, UI-S-017, UI-S-016 and Finding 4
- [x] Review UI-F2.1a, UI-F2.8 and UI-F3.1 (holistic-ui): one tree element, same data interface, sections and item index
- [ ] Measure the Large open-folder and back classes before the change, with the machine line

## Failing tests first

- [ ] Unit: window math keeps the bound over several sections (gate 1)
- [ ] Unit: rows carry level, set size and position over the whole level, parents and limit rows (gates 7, 8, 9)
- [ ] Unit: `readLevel` stops at 10,000 nodes or 50 pages, says so and reports the cap (gates 3, 4)
- [ ] UI: a level of 10,000 nodes keeps at most the window plus two overscans of 5 tree items (gate 1)
- [ ] UI: arrow keys, Home, End, expand and collapse reach undrawn nodes (gate 11)
- [ ] UI: collapse state of 1,000 nodes and pins, a pin beyond the cap included, survive a reload (gates 12, 13)
- [ ] UI: a capped level shows the `navigationLevelLimit` line and the console message (gates 4, 5)
- [ ] Run them and record that they fail

## Gates

- [ ] 1. DOM bound: window plus two overscans of 5 (unit, UI, preview and performance tests)
- [ ] 2. Scrolling area no taller than the sidebar (preview and performance tests)
- [ ] 3. Level cap of 10,000 nodes, 50 pages of 200
- [ ] 4. Console message at the cap
- [ ] 5. Limit line from `navigationLevelLimit` in both label tables, "Showing the first 10,000 only. Use search to find the rest."
- [ ] 6. Tree and tree item roles
- [ ] 7. `aria-level` on every item
- [ ] 8. `aria-setsize` and `aria-posinset` over the whole level
- [ ] 9. `aria-expanded` on parents, `aria-current` on the current destination
- [ ] 10. One Tab stop in each section's tree, no positive `tabindex`
- [ ] 11. Keyboard reaches every node of an open level; an undrawn node is drawn before it takes focus
- [ ] 12. Collapse state, at most 1,000 nodes, survives a reload
- [ ] 13. Pins, at most 100, survive a reload; a pin beyond the cap opens by handle; a `not_found` pin is unavailable with Remove
- [ ] 14. Sections by kind with Recently deleted as the final row (mockup 55)
- [ ] 15. One tree element: data interface, sections and item index kept
- [ ] 16. Known gap removed from the performance suite, the unit test, the feature file and the README
- [ ] 17. Large open-folder and back result p95 ≤ 100 ms, p99 ≤ 200 ms
- [ ] 18. Large open-folder and back feedback p95 ≤ 100 ms, p99 ≤ 200 ms
- [ ] 19. The same gates under 5 `navigation.change` events a second
- [ ] 20. 120 samples after a warm-up of 20, 5,000 ms sample timeout, idle control: nothing loosened
- [ ] 21. Row look: 30 px rows, 13 px text, 20 px indent, 16 px icon, active tint and brand bar, tokens in light and dark
- [ ] 22. UI-S-016 contracts hold (shared stylesheet, focus ring, logical properties, motion, no pixel heights)
- [ ] 23. Playwright against the production build with strict DOM assertions

## Finish

- [ ] TSDoc on every new export; functions under 60 lines
- [ ] `npx prettier --write .`, `npx eslint . --fix`, `npx tsc --noEmit`, `npm test`
- [ ] Preview Playwright suite under the lock
- [ ] Full performance suite once, under the lock; numbers before and after in the Walkthrough
- [ ] README, SRS, plan row, feature files updated
- [ ] Walkthrough written; Plan and ToDos deleted; boxes ticked in the UI plan and the checklist (not the FS-3.4 global box); `python3 scripts/docs/update_mvp_progress.py`
- [ ] `bash scripts/pre-commit.sh` passes with `Test scope: npm:apps/filesquid/ui`
- [ ] Merge `origin/main`, push, open the pull request, run the review loop
