# Task build-checklist-in-progress-up-next: Walkthrough

## Owner-requested docs-tooling change (2026-10-18): In progress and Up next under the MVP progress bar

No PRD or SRS requirement. The generated block at the top of [implementation-plan-checklist.md](../implementation-plan-checklist.md) now carries two more sections, drawn by [`update_mvp_progress.py`](../../scripts/docs/update_mvp_progress.py) and checked by [`verify_docs.sh`](../../scripts/verify_docs.sh).

## What changed

| File | Change |
|---|---|
| [`scripts/docs/mvp_queue.py`](../../scripts/docs/mvp_queue.py) | New. Pure logic: checklist and plan parsing, Dependencies parsing, PR-to-task matching, In progress and Up next selection, rendering. No file, network or clock access. |
| [`scripts/docs/test_mvp_queue.py`](../../scripts/docs/test_mvp_queue.py) | New. 27 stdlib `unittest` tests, written first. |
| [`scripts/docs/update_mvp_progress.py`](../../scripts/docs/update_mvp_progress.py) | Fetches open PRs with `gh api` in write mode only (30 s timeout; on failure keeps the recorded PRs and prints a warning naming the cause to stderr). Renders the new sections and a UTC timestamp. |
| [`scripts/verify_docs.sh`](../../scripts/verify_docs.sh) | Runs the unit tests as a new docs-gate step. |
| [`implementation-plan-checklist.md`](../implementation-plan-checklist.md) | Regenerated block; the explanatory sentence describes the new sections. At the owner's request the line "Superseded tasks (0.7, 1.9, 9.5, 10.4 and their superseded module rows) are left out. ImageSquid (`IS-`) and Arkrim (`ARK-`) view rows ..." is removed. Nothing else reproduces or checks that sentence, and no listed row changed. |

## Rules

- In progress: a global task with some but not all module rows ticked (its unticked rows are listed), plus any unticked task whose ID appears in the head branch or title of an open PR. IDs are matched as whole tokens against the real unticked IDs in the checklist (`.` may be `-`; `IO-0.1` never matches inside `IO-0.10`; a bare global number such as `0.4` needs `task` before it).
- Up next: the first five unticked, not-in-progress MVP module rows in checklist order whose Dependencies are all ticked or in progress. `global Task N` means all of that task's module rows. An ID found nowhere is not ready; an unparseable or missing Dependencies line is not ready and warns on stderr.
- Determinism: `--check` never calls GitHub. The PR-derived IDs and the timestamp are embedded in the block as `<!-- in-progress-prs: ... -->` and `<!-- generated-at: ... -->` and read back; everything else is recomputed from ticks and plans. Write mode keeps the old timestamp when nothing else changed.

## Test run (raw output)

```text
$ python3 scripts/docs/test_mvp_queue.py
...........................
----------------------------------------------------------------------
Ran 27 tests in 0.004s

OK

$ python3 scripts/docs/update_mvp_progress.py --check
$ echo $?
0
```
