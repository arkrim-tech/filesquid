# Task build-checklist-in-progress-up-next: Walkthrough

## Owner-requested docs-tooling change (2026-10-18): In progress and Up next under the MVP progress bar

No PRD or SRS requirement. The generated block at the top of [implementation-plan-checklist.md](../implementation-plan-checklist.md) now carries two more sections, drawn by [`update_mvp_progress.py`](../../scripts/docs/update_mvp_progress.py) and checked by [`verify_docs.sh`](../../scripts/verify_docs.sh).

## What changed

| File | Change |
|---|---|
| [`scripts/docs/mvp_queue.py`](../../scripts/docs/mvp_queue.py) | New. Pure logic: checklist and plan parsing, Dependencies parsing, PR-to-task matching, In progress and Up next selection, rendering. No file, network or clock access. |
| [`scripts/docs/test_mvp_queue.py`](../../scripts/docs/test_mvp_queue.py) | New. stdlib `unittest` tests, written first. |
| [`scripts/docs/update_mvp_progress.py`](../../scripts/docs/update_mvp_progress.py) | Fetches open PRs with `gh api` in write mode only (30 s timeout; on failure keeps the recorded PRs and prints a warning naming the cause to stderr). Renders the new sections and a UTC timestamp. |
| [`scripts/verify_docs.sh`](../../scripts/verify_docs.sh) | Runs the unit tests as a new docs-gate step. |
| [`implementation-plan-checklist.md`](../implementation-plan-checklist.md) | Regenerated block; the explanatory sentence describes the new sections. At the owner's request the line "Superseded tasks (0.7, 1.9, 9.5, 10.4 and their superseded module rows) are left out. ImageSquid (`IS-`) and Arkrim (`ARK-`) view rows ..." is removed from the checklist. No script or docs check reproduces or depends on that sentence, and no listed row changed. [implementation-plan.md](../implementation-plan.md) keeps its own statement that it sequences FileSquid only and does not dispatch ImageSquid rows; it is untouched. |

## Rules

- In progress: a global task with some but not all module rows ticked (its unticked rows are listed), plus any unticked task whose ID appears in the head branch or title of an open PR. IDs are matched as whole tokens against the real unticked IDs in the checklist (`.` may be `-`; `IO-0.1` never matches inside `IO-0.10`; a bare global number such as `0.4` needs a `task` token or an exact `<type>/<id>` branch prefix such as `feat/0.4-write`, so `release/0.4-rc` does not match).
- Up next: the first five unticked, not-in-progress MVP module rows in checklist order whose Dependencies are all ticked or in progress. `global Task N` means all of that task's module rows. An ID found nowhere is not ready; an unparseable or missing Dependencies line is not ready and warns on stderr.
- Determinism: `--check` never calls GitHub. The PR-derived IDs and the timestamp are embedded in the block as `<!-- in-progress-prs: ... -->` and `<!-- generated-at: ... -->` and read back; everything else is recomputed from ticks and plans. Write mode keeps the old timestamp when nothing else changed.

## Test run (raw output)

```text
$ python3 scripts/docs/test_mvp_queue.py
...................................
----------------------------------------------------------------------
Ran 35 tests in 0.007s

OK

$ python3 scripts/docs/update_mvp_progress.py --check
$ echo $?
0
```

## Merged tasks and the offline check

When a task PR merges, its task is ticked. `--check` reads the recorded PR IDs back unchanged, lists only PR-derived tasks that are still unticked, and recomputes everything else from the ticks. A recorded PR whose task has since been ticked is therefore never listed, and it does not by itself make the block stale. Any PR that ticks boxes already has to rerun `python3 scripts/docs/update_mvp_progress.py` (the percentage and the task lists change with the ticks); a PR that ticks nothing never touches the block. Opening or closing a PR never affects `--check`. Main was merged on 2026-10-18 after PR #48 (IO-0.9) and the script rerun to resolve the block conflict.
