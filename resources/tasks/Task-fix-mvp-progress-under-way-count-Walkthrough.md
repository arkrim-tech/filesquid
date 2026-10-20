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
