# Task build-checklist-merge-driver: Walkthrough

## Owner-approved build change (2026-10-18): a git merge driver for the MVP progress block

No PRD or SRS requirement. Every task PR ticks boxes in [implementation-plan-checklist.md](../implementation-plan-checklist.md) and must commit a regenerated progress block and bar ([mvp-progress.svg](../assets/mvp-progress.svg)), because [`verify_docs.sh`](../../scripts/verify_docs.sh) fails while they are stale. Two open PRs therefore conflicted on those generated lines whenever one merged. A custom git merge driver now merges them.

## What changed

| File | Change |
|---|---|
| [`.gitattributes`](../../.gitattributes) | New. `merge=mvp-progress` for the checklist and the SVG. |
| [`scripts/docs/merge_mvp_progress.py`](../../scripts/docs/merge_mvp_progress.py) | New. The driver (`%O %A %B %L %P`). |
| [`scripts/docs/test_merge_mvp_progress.py`](../../scripts/docs/test_merge_mvp_progress.py) | New. stdlib `unittest` tests that run real `git merge`s in throwaway repositories. Written before the driver. |
| [`scripts/docs/update_mvp_progress.py`](../../scripts/docs/update_mvp_progress.py) | Small refactor so the driver reuses it: `expected_outputs` (the body of `main`, shared with the driver), `percent_done`, and a hidden SVG tick record (`tick_record`, `parse_tick_record`). The checklist's explanatory sentence names the driver. `mvp_queue.py` is unchanged. |
| [`scripts/setup-dev-environment.sh`](../../scripts/setup-dev-environment.sh) | New optional step 5: registers the driver in the clone's git config. It is idempotent: a second run logs `merge.mvp-progress.driver already set`. |
| [`scripts/verify_docs.sh`](../../scripts/verify_docs.sh) | The existing `unittest discover` step now also runs the driver tests; label and header updated. |
| [`README.md`](../../README.md) | Setup: how to register the driver, what it does, its limits and the fallback. Run tests: the docs test commands. |
| [`implementation-plan-checklist.md`](../implementation-plan-checklist.md), [`mvp-progress.svg`](../assets/mvp-progress.svg) | Regenerated offline. The checklist only gains the new sentence (the recorded PRs and timestamp are unchanged). The SVG gains its tick record. |

## How the driver works

- **Checklist.** The generated block in base, ours and theirs is replaced by a fixed placeholder block. The rest is merged with one `git merge-file -p` call, with a 60-second limit, in a temporary directory that is removed on every exit path. If that merge is clean, the block is regenerated offline (no GitHub call) with `update_mvp_progress.expected_outputs` in `--check` mode. The recorded PR IDs are the union of both sides' `in-progress-prs`; when one task has two PRs, the lower number wins, as in `match_pr_ids`. The union is then filtered by `mvp_queue.only_unticked`, and the timestamp is the newer of the two `generated-at` values. The result goes to `%A` and the driver exits 0. If the rest has a real conflict, `%A` keeps git's markers there with ours' block in place of the placeholder, and the driver exits 1 so git reports the conflict. If `git merge-file` fails or the block cannot be regenerated, the driver also exits 1 and prints the cause.
- **SVG.** Every SVG now carries `<metadata id="mvp-ticks">`, which is not drawn. It holds one line per MVP global task, for example `Task 0.4 [ ] 48h: IO-0.6 [x] … IO-0.10 [ ]`. The driver reads the three records, picks the row structure by three-way choice, merges every tick and estimate row by row (a side that changed wins; two different changes, or a change to a row the other side removed, are a clash), and redraws the bar with `render_svg`. The SVG driver never reads the checklist, and the checklist driver never writes the SVG, so the result does not depend on which file git merges first. When the checklist merge is clean, the row-by-row merge gives the same ticks as the merged checklist, so the bar matches the regenerated block. A version without a tick record, or a clash, keeps ours and exits 1 with a message to rerun `python3 scripts/docs/update_mvp_progress.py`.
- **Order.** Git 2.43 visits both files in reverse path order under both `ort` and `recursive` (checked with a probe driver). The design does not rely on that order: the tests run both strategies, and they also call the two drivers directly in both orders.

## Limits

- **Plans and estimates come from the working tree.** The block's Up next list reads the module plans' Dependencies and ticks, and the hours come from [implementation-plan.md](../implementation-plan.md). While git merges, the working tree still holds ours. When theirs changed one of those, the block (or the bar's hours) can be stale after the merge. The pre-commit hook's `update_mvp_progress.py --check` reports it, and `python3 scripts/docs/update_mvp_progress.py` fixes it.
- **Adjacent ticks conflict.** `git merge-file` treats edits on adjacent lines as a conflict, so two branches that tick neighbouring checklist rows still conflict there. That conflict is in the rest of the file, not the block. Resolve it, then rerun the script.
- **Older branches.** A branch whose own tree has no `.gitattributes` entry gets a normal text merge under `ort`, because the attributes come from that side. Under the legacy `recursive` strategy, git 2.43 already runs the driver. A version whose SVG has no tick record (older than this change) makes the SVG driver keep ours and report a conflict, so that the stale bar is never committed silently.
- **No driver configured.** Without `merge.mvp-progress.driver`, git ignores the undefined driver name and uses its builtin text merge, so both files conflict as before. Test (e) proves this.

## Test run (raw output)

```text
$ python3 -m unittest -v scripts/docs/test_merge_mvp_progress.py
test_both_strategies_give_the_same_result (scripts.docs.test_merge_mvp_progress.CleanMerge.test_both_strategies_give_the_same_result)
The default strategy and `-s recursive` (which may visit files in another order) agree. ... ok
test_different_ticks_merge_into_the_offline_block (scripts.docs.test_merge_mvp_progress.CleanMerge.test_different_ticks_merge_into_the_offline_block)
The merged block and SVG equal what the offline generator draws from the merged ticks. ... ok
test_driver_call_order_does_not_matter (scripts.docs.test_merge_mvp_progress.CleanMerge.test_driver_call_order_does_not_matter)
Running the SVG driver before or after the checklist driver gives the same files. ... ok
test_merge_direction_does_not_matter (scripts.docs.test_merge_mvp_progress.CleanMerge.test_merge_direction_does_not_matter)
Merging a into b gives the same checklist and SVG as merging b into a. ... ok
test_merge_from_a_subdirectory (scripts.docs.test_merge_mvp_progress.CleanMerge.test_merge_from_a_subdirectory)
Git runs the driver from the top-level directory, so its relative path works anywhere. ... ok
test_same_line_edited_differently_conflicts (scripts.docs.test_merge_mvp_progress.RealConflict.test_same_line_edited_differently_conflicts)
The merge fails, markers stay around the edited line and the block is valid. ... ok
test_union_filtered_by_ticks (scripts.docs.test_merge_mvp_progress.RecordedPrs.test_union_filtered_by_ticks)
A recorded PR for a task the other side ticked is dropped. ... ok
test_pick_three_way (scripts.docs.test_merge_mvp_progress.Unit.test_pick_three_way)
A side that changed wins; two different changes conflict. ... ok
test_rows_changed_differently_do_not_merge (scripts.docs.test_merge_mvp_progress.Unit.test_rows_changed_differently_do_not_merge)
The same row ticked on one side and changed in hours on the other still merges; a clash does not. ... ok
test_tick_record_round_trip (scripts.docs.test_merge_mvp_progress.Unit.test_tick_record_round_trip)
The SVG tick record reads back to the same tasks; an SVG without one reads as None. ... ok
test_union_keeps_lowest_pr_and_newer_time (scripts.docs.test_merge_mvp_progress.Unit.test_union_keeps_lowest_pr_and_newer_time)
The same task recorded with two PRs keeps the lower number. ... ok
test_undefined_driver_is_the_builtin_text_merge (scripts.docs.test_merge_mvp_progress.WithoutConfig.test_undefined_driver_is_the_builtin_text_merge)
Both branches redraw the block, so the plain text merge conflicts there. ... ok

----------------------------------------------------------------------
Ran 12 tests in 6.021s

OK```

The tests map to the owner's list as follows: (a) `test_different_ticks_merge_into_the_offline_block`; (b) `test_both_strategies_give_the_same_result`, `test_driver_call_order_does_not_matter` and `test_merge_direction_does_not_matter`; (c) `test_same_line_edited_differently_conflicts`; (d) `test_union_filtered_by_ticks`; (e) `test_undefined_driver_is_the_builtin_text_merge`. Each test builds its repository with `tempfile.mkdtemp` and removes it through `addCleanup`. Git runs with `GIT_CONFIG_GLOBAL` set to the null device and `GIT_CONFIG_NOSYSTEM=1`.

A dry run on the real checklist (outside the test suite) ticked IO-0.10 on one side and DE-0.3 on the other, each side recording its own PR. Both drivers exited 0, `--check` matched both merged files, the bar read 44%, and the two PRs recorded for the now-ticked tasks were dropped from `in-progress-prs`.

```text
$ bash scripts/setup-dev-environment.sh      # second run, from the repository root
[2026-10-18T20:39:52Z] ▶ Git merge driver for the MVP progress block (optional)
[2026-10-18T20:39:52Z] merge.mvp-progress.driver already set
[2026-10-18T20:39:52Z] ✅ Git merge driver for the MVP progress block (0s)
[2026-10-18T20:39:52Z] ✅ Setup complete (14s)

$ git config merge.mvp-progress.name "MVP progress block"
$ git config merge.mvp-progress.driver "python3 scripts/docs/merge_mvp_progress.py %O %A %B %L %P"
$ git config --get-regexp '^merge\.mvp-progress'
merge.mvp-progress.name MVP progress block
merge.mvp-progress.driver python3 scripts/docs/merge_mvp_progress.py %O %A %B %L %P
```

## First real use: merging main into this branch

Main moved on (PRs #47 and #56 merged) while the PR was open, with the driver registered in this clone. `git merge origin/main` merged the checklist cleanly through the driver: the regenerated block passed `--check`. The SVG driver reported a conflict, as documented, because main's bar predates the tick record:

```text
mvp-progress merge driver: an SVG version has no tick record; keeping ours. Run python3 scripts/docs/update_mvp_progress.py after the merge.
Auto-merging resources/assets/mvp-progress.svg
CONFLICT (content): Merge conflict in resources/assets/mvp-progress.svg
Auto-merging resources/implementation-plan-checklist.md
```

The bar was redrawn offline to resolve that conflict. This only happens once per branch: after this PR merges, every base SVG carries a tick record.

## Review fixes (Cubic round 1)

- `Repo.tick` in the tests now ticks global rows (`[Task 0.2]`) as well as module rows, and fails when no open row matches, so `test_both_strategies_give_the_same_result` really ticks Task 0.2.
- `parse_tick_record` rejects a record line whose module part does not fully parse, so a corrupt SVG leads to the conflict fallback.
- The setup step skips registration (a warning from an optional step) when `python3` is missing, and its comment says a different driver value is replaced.
- The checklist sentence now says the driver works in clones that register it, and leaves a conflict when it cannot merge.

## Pre-commit hook

The change scope is `full`, because `setup-dev-environment.sh`, a new script and the root `.gitattributes` changed. The hook ran with `CARGO_TARGET_DIR` set to a shared target, `CARGO_INCREMENTAL=0` and `CARGO_PROFILE_DEV_DEBUG=0`.

```text
$ bash scripts/pre-commit.sh
✅ All Pre-Commit Hooks Passed! Test scope: full (214s)
```
