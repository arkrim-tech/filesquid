# Task chore-task-cleanup-root: Walkthrough

Owner-approved chore. The task-cleanup gate ([`scripts/verify_task_cleanup.sh`](../../scripts/verify_task_cleanup.sh), run by [`scripts/verify_docs.sh`](../../scripts/verify_docs.sh)) only searched `apps/` for Walkthroughs, so a Plan or ToDos file left next to a Walkthrough in the repository-root `resources/tasks/` was never reported.

## Changes

- Deleted the abandoned `resources/tasks/Task-ui-mockups-v1-Plan.md` and `resources/tasks/Task-ui-mockups-v1-ToDos.md`. The owner confirmed they were replaced by [Task ui-mockups v2](./Task-ui-mockups-v2-Walkthrough.md). No Markdown link pointed at them. One plain-text mention remains in the historical [Task MD-0.5 walkthrough](../../apps/filesquid/crates/metadata/resources/tasks/Task-MD-0.5-Walkthrough.md); it records what happened on that branch and is left as written.
- `scripts/verify_task_cleanup.sh` now looks for `*-Walkthrough.md` under `apps/` and under `resources/tasks/`. The rule is unchanged: a `-Plan.md` or `-ToDos.md` with the same task ID next to a Walkthrough fails. A Plan or ToDos without a Walkthrough is still allowed (work in progress). File names are read NUL-separated, so names with spaces are safe.
- Header comments in the script and in `scripts/verify_docs.sh` name both locations.
- No file in `.agents/rules/` states that the check is apps-only, so none was edited. No test harness covers the hook scripts (`scripts/verify_changed_scope.sh` tests the scope helper and libraries only), so the root-folder case was verified by hand below.

## Manual fixture verification

Temporary fixture with `apps/x/resources/tasks/` and `resources/tasks/`, run with the script from `main` (old) and the changed script (new):

```text
== old script: root Plan next to Walkthrough (name has a space), plus orphan Plan
exit=0
== new script: same fixture
❌ ERROR: Task cleanup failure. Found Walkthrough but Plan document still exists: resources/tasks/Task-a b-Plan.md
You must remove the '-Plan.md' and '-ToDos.md' files once the '-Walkthrough.md' is generated.
exit=1
== new script: apps ToDos violation still caught
❌ ERROR: Task cleanup failure. Found Walkthrough but ToDos document still exists: apps/x/resources/tasks/Task-1-ToDos.md
You must remove the '-Plan.md' and '-ToDos.md' files once the '-Walkthrough.md' is generated.
exit=1
== new script: clean fixture (orphan Plan without Walkthrough is allowed)
exit=0
```

## Hook run

`FULL=1 bash scripts/pre-commit.sh`:

```text
✅ Task cleanup passed.
✅ Documentation validation passed.
✅ Lint and test validation passed (test scope: full).
========================================
✅ All Pre-Commit Hooks Passed! Test scope: full (696s)
========================================
```
