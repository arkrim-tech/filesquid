---
trigger: always_on
description: Run every agent pull request through the Cubic review loop until no Cubic comments remain, then merge it into main.
copyright: Pieter Van Garderen (2026)
license: CC-BY
---

# Pull Request Review Loop (Cubic)

**Purpose:** This rule enforces the PR Review Mandate in `AGENTS.md`. The external reviewer is Cubic (`cubic-dev-ai[bot]` on GitHub). While agents work independently towards the MVP, the owner has granted them permission (2026-10-17) to merge their own pull requests into `main` once this loop is finished.

## The loop

1. **Open the PR.** When a task is complete and `bash scripts/pre-commit.sh` exits 0, push the task branch and open a pull request against `main`. Follow [pr-documentation-update.md](./pr-documentation-update.md) first.
2. **Wait for Cubic.** Subscribe to the PR's activity so Cubic's review wakes the agent the moment it is posted, and keep a fallback check-in about 10 minutes out in case an event is lost. Do not poll with `sleep`. Cubic reviews each new head commit on its own; it is done when its `cubic · AI code reviewer` check run on that commit has completed (a clean pass posts no new review). If that check run has not completed 15 minutes after the push, re-trigger Cubic with the "Re-trigger cubic" link in its last review or by mentioning `@cubic-dev-ai` in a PR comment. Re-trigger at most 3 times per head commit; if Cubic still has not reviewed it, stop, do not merge, and report the blocker to the owner.
3. **Fix every Cubic comment.** For each open Cubic review thread:
   - Fix it in code or documentation, following every other rule in `.agents/rules/` (TDD, lint, docs, links).
   - If the finding is wrong, reply on the thread with the reasoning and evidence, then resolve the thread. Never dismiss a finding silently.
   - If you disagree with a security finding, leave the thread open and ask the owner.
4. **Push the fixes.** Rerun `bash scripts/pre-commit.sh`, commit with Conventional Commits, and push to the same branch. Reply on each thread you fixed, naming the commit, and resolve it.
5. **Repeat** steps 2 to 4 until the merge conditions below hold.

## Merge conditions

Merge the PR into `main` only when all of these are true:

- Cubic has reviewed the current head commit: its `cubic · AI code reviewer` check run on that commit has completed with `success`. A clean pass posts no new review, so the check run is the signal; new findings arrive as review comments.
- No Cubic review thread is unresolved.
- `bash scripts/pre-commit.sh` exited 0 on the current head commit.
- The PR has no merge conflict with `main`. Resolve a conflict by merging `main` into the branch, then return to step 2.
- No human reviewer has requested changes that are still open.

Use a merge commit (the repository's existing convention), then delete the task branch on the remote if it is not the agent's assigned session branch.

## Bound

The loop is capped at **10 Cubic review rounds** per PR (`AGENTS.md` §22). If Cubic still has open comments after round 10, or the same finding returns three times after being fixed, stop. Do not merge. Report to the owner what is still open and why it keeps coming back.

## MUST NOT

- Merge with an unresolved Cubic thread, a failing pre-commit hook, or before Cubic's check run on the head commit has completed.
- Commit or push directly to `main`. The PR merge is the only way agent work reaches `main` ([task-workflow.md](./task-workflow.md)).
- Weaken a verifier, skip a test, or bypass the hook with `--no-verify` to end the loop ([local-verification.md](./local-verification.md)).
