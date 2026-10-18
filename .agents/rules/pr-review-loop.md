---
trigger: always_on
description: Run every agent pull request through the Cubic review loop until no Cubic comments remain, then merge it into main.
copyright: Pieter Van Garderen (2026)
license: CC-BY
---

# Pull Request Review Loop (Cubic)

**Purpose:** This rule enforces the PR Review Mandate in `AGENTS.md`. The external reviewer is Cubic (`cubic-dev-ai[bot]` on GitHub). The owner has granted agents permission (2026-10-17) to merge their own pull requests into `main` once this loop is finished, for tasks inside the current milestone only.

## Current milestone

Only the owner edits this section. It is updated after the owner's manual testing review of the previous milestone, to open the next development phase.

| Field | Value |
|---|---|
| Milestone | FileSquid MVP (macOS desktop, unsigned early-access DMG; [ADR 035](../../resources/architecture/035-macos-first-desktop-mvp.md)) |
| Scope | Every task in phases F0, F1, F2 and F3 of [resources/implementation-plan.md](../../resources/implementation-plan.md) |
| Gate task | **Task FS-3.9: FileSquid MVP gate (macOS desktop)** |
| Opened | 2026-10-17 |

1. **Inside the milestone:** agents MAY pick up, implement and self-merge tasks in the scope above, in the plan's dependency order.
2. **Outside the milestone:** agents MUST NOT start a task outside the scope (for example phase F3R, F4 or I1) on their own. If the owner explicitly asks for one, run the loop below but do not merge; leave the PR for the owner.
3. **Stop at the gate:** after the gate task's PR is merged, stop. Do not start another task. Report to the owner that the milestone is complete and ready for manual testing review, with links to the gate evidence and any open gaps.
4. **Next milestone:** work resumes only after the owner replaces the table above with the next milestone's scope and gate task.

## The loop

1. **Open the PR.** When a task is complete and the full suite passes (`FULL=1 bash scripts/pre-commit.sh` exits 0 and its summary line reads `Test scope: full`; see [local-verification.md](./local-verification.md#change-aware-test-scope)), push the task branch and open a pull request against `main`. Follow [pr-documentation-update.md](./pr-documentation-update.md) first.
2. **Wait for Cubic.** Subscribe to the PR's activity so Cubic's review wakes the agent the moment it is posted, and keep a fallback check-in about 10 minutes out in case an event is lost. Do not poll with `sleep`. Cubic reviews each new head commit on its own; it is done when its `cubic · AI code reviewer` check run on that commit has completed (a clean pass posts no new review). If that check run has not completed 15 minutes after the push, re-trigger Cubic with the "Re-trigger cubic" link in its last review or by mentioning `@cubic-dev-ai` in a PR comment. Re-trigger at most 3 times per head commit; if Cubic still has not reviewed it, stop, do not merge, and report the blocker to the owner.
3. **Fix every Cubic comment.** For each open Cubic review thread:
   - Fix it in code or documentation, following every other rule in `.agents/rules/` (TDD, lint, docs, links).
   - If the finding is wrong, reply on the thread with the reasoning and evidence, then resolve the thread. Never dismiss a finding silently.
   - If you disagree with a security finding, leave the thread open and ask the owner.
4. **Push the fixes.** Rerun `bash scripts/pre-commit.sh` (its change-aware scope is enough for a fix commit), commit with Conventional Commits, and push to the same branch. Reply on each thread you fixed, naming the commit, and resolve it.
5. **Repeat** steps 2 to 4 until the merge conditions below hold.

## Merge conditions

Merge the PR into `main` only when all of these are true:

- Cubic has reviewed the current head commit: its `cubic · AI code reviewer` check run on that commit has completed with `success`. A clean pass posts no new review, so the check run is the signal; new findings arrive as review comments.
- No Cubic review thread is unresolved.
- `FULL=1 bash scripts/pre-commit.sh` exited 0 on the current head commit, with the summary line `Test scope: full`. A change-aware run is not enough for the final commit.
- The PR has no merge conflict with `main`. Resolve a conflict by merging `main` into the branch, then return to step 2.
- No human reviewer has requested changes that are still open.

Use a merge commit (the repository's existing convention), then delete the task branch on the remote if it is not the agent's assigned session branch.

## Bound

The loop is capped at **10 Cubic review rounds** per PR (`AGENTS.md` §22). If Cubic still has open comments after round 10, or the same finding returns three times after being fixed, stop. Do not merge. Report to the owner what is still open and why it keeps coming back.

## MUST NOT

- Merge with an unresolved Cubic thread, a failing pre-commit hook, or before Cubic's check run on the head commit has completed.
- Commit or push directly to `main`. The PR merge is the only way agent work reaches `main` ([task-workflow.md](./task-workflow.md)).
- Weaken a verifier, skip a test, or bypass the hook with `--no-verify` to end the loop ([local-verification.md](./local-verification.md)).
