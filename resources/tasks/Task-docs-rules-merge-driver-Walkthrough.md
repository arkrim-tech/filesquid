# Task docs-rules-merge-driver Walkthrough: merge driver in the agent rules

Documentation-only task on branch `docs/rules-merge-driver`. TDD-ENFORCE: Not applicable - documentation update.

## Owner request (2026-10-18)

The owner asked for two edits to `.agents/rules/`, and nothing else there.

## Changes

- [task-workflow.md](../../.agents/rules/task-workflow.md): after each sentence that says to run `update_mvp_progress.py` and that `verify_docs.sh` fails while it is stale (global and module prompt paragraphs), the owner-specified merge-driver registration and conflict-recovery sentences.
- [local-verification.md](../../.agents/rules/local-verification.md): step 8 row of "The gates" table now names the MVP progress merge-driver tests.

The root README ("Run tests" and setup sections) already documents the two `git config` commands (`merge.mvp-progress.name` and `merge.mvp-progress.driver`), so the wording "the two `git config` commands in the README" is accurate.

## Hook

Ran `bash scripts/pre-commit.sh` on the head commit. Summary line: `✅ All Pre-Commit Hooks Passed! Test scope: docs`.
