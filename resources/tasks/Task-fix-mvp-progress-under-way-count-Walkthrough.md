# Task fix-mvp-progress-under-way-count Walkthrough

Root cause: the summary counted every open task with at least one ticked module row as "under way". That included Task 0.8, whose module rows are all ticked but whose own box is open, so it is correctly absent from the In progress list (the list needs some but not all rows ticked, or an open pull request). The summary also ignored tasks listed In progress only because of an open pull request. Separately, the `(N of M done)` count under `## MVP` was hand-written and stale (13 instead of 14).

Fix: the summary's under-way count is now the number of open global tasks in the In progress list, and the `(N of M done)` count under `## MVP` is generated from the ticks by `update_mvp_progress.py`.

Raw output of `python3 -m unittest discover -s scripts/docs -p 'test_*.py'`:

```text
...............................................
----------------------------------------------------------------------
Ran 47 tests in 6.038s

OK
```

Raw output of `python3 scripts/docs/update_mvp_progress.py --check`:

```text
exit=0
```

## Cubic round 1: flat PR-only entries

Finding (P2): `sum(entry.nested for entry in inprog.entries)` ignored a PR on a module of an otherwise untouched global task, which `compute_in_progress` lists as a flat `nested=False` entry, so the summary undercounted the In progress list.

Fix: `mvp_queue.count_under_way` counts the distinct unticked global tasks (`entry.global_row.task_id` where `entry.global_row.done` is false) among all In progress entries. Two flat entries under one global count it once; a flat entry under an already ticked global is not counted. `update_mvp_progress.build_queue` uses it. The current checklist output is unchanged (no flat entries today).

Red: raw output of `python3 -m unittest test_mvp_queue` (in `scripts/docs`) before the fix, paths shortened to the repository:

```text
.........EE..........................
======================================================================
ERROR: test_under_way_counts_flat_pr_only_entries (test_mvp_queue.InProgress.test_under_way_counts_flat_pr_only_entries)
A PR on a module of an untouched global counts its global once as under way.
----------------------------------------------------------------------
Traceback (most recent call last):
  File "scripts/docs/test_mvp_queue.py", line 211, in test_under_way_counts_flat_pr_only_entries
    self.assertEqual(q.count_under_way(result), 2)
                     ^^^^^^^^^^^^^^^^^
AttributeError: module 'mvp_queue' has no attribute 'count_under_way'

======================================================================
ERROR: test_under_way_skips_ticked_globals (test_mvp_queue.InProgress.test_under_way_skips_ticked_globals)
A flat entry under a ticked global is not counted; that global is already done.
----------------------------------------------------------------------
Traceback (most recent call last):
  File "scripts/docs/test_mvp_queue.py", line 218, in test_under_way_skips_ticked_globals
    self.assertEqual(q.count_under_way(result), 1)
                     ^^^^^^^^^^^^^^^^^
AttributeError: module 'mvp_queue' has no attribute 'count_under_way'

----------------------------------------------------------------------
Ran 37 tests in 0.007s

FAILED (errors=2)
```

The old formula on the first new test's fixture (In progress 0.4 nested, 0.5 flat twice):

```text
old formula sum(entry.nested): 1
```

Green: raw output of `python3 -m unittest discover -s scripts/docs -p 'test_*.py'` after the fix:

```text
.................................................
----------------------------------------------------------------------
Ran 49 tests in 6.146s

OK
```

Raw output of `python3 scripts/docs/update_mvp_progress.py --check`:

```text
exit=0
```
