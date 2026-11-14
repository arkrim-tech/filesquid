---
trigger: always_on
description: Open a pull request, pass the local hook, fix open human review comments, then merge it into main.
copyright: Pieter Van Garderen (2026)
license: CC-BY
---

# Pull Request Review Loop

**Purpose:** This rule enforces the PR Review Mandate in `AGENTS.md`. There is no hosted pull-request reviewer (owner decision, 2026-11-14: the AI PR Agent workflow was removed). The owner has granted agents permission (2026-10-17) to merge their own pull requests into `main` once this loop is finished, for tasks inside the current milestone only.

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

1. **Open the PR.** When a task is complete and the head-commit hook run required by its scope passes (`FULL=1 bash scripts/pre-commit.sh`, summary line `Test scope: full`, when the PR's change scope is `full` or includes `rust:`; a passing ordinary `bash scripts/pre-commit.sh` when it is `docs` or npm-only; see [local-verification.md](./local-verification.md#change-aware-test-scope)), push the task branch and open a pull request against `main`. Follow [pr-documentation-update.md](./pr-documentation-update.md) first.
2. **No hosted reviewer.** Do not poll a GitHub Actions check run, and do not comment `/review`. After each push, read the PR's `mergeable_state`; when it is `dirty`, merge `main` into the branch straight away. Agents MUST NOT subscribe to PR activity or create scheduled triggers, wake-ups or check-ins (owner decision, 2026-10-18).
3. **Fix every human review comment that requests a change.** For each open human review thread:
   - Fix it in code or documentation, following every other rule in `.agents/rules/` (TDD, lint, docs, links).
   - If the finding is wrong, reply on the thread with the reasoning and evidence, then resolve the thread. Never dismiss a finding silently.
   - Each thread gets exactly one reply: the fix commit or the reasoning.
   - If you disagree with a security finding, leave the thread open and ask the owner.
4. **Push the fixes.** Rerun `bash scripts/pre-commit.sh` (its change-aware scope is enough for a fix commit), commit with Conventional Commits, and push to the same branch. Reply on each thread you fixed, naming the commit, and resolve it. Resolve the thread with the host's review API. When GraphQL is available, use the `resolveReviewThread` mutation.
5. **Repeat** steps 2 to 4 until the merge conditions below hold.

## Merge conditions

Merge the PR into `main` only when all of these are true:

- The hook passed on the current head commit with the run its scope requires: `FULL=1 bash scripts/pre-commit.sh`, summary line `Test scope: full`, when the PR's change scope (`scripts/changed_scope.sh` against `origin/main`) is `full` or includes `rust:`; otherwise a passing ordinary `bash scripts/pre-commit.sh`, summary line `Test scope: <scope>`.
- The PR has no merge conflict with `main`. Resolve a conflict by merging `main` into the branch, then return to step 2.
- No human reviewer has requested changes that are still open.

Use a merge commit (the repository's existing convention), then delete the task branch on the remote if it is not the agent's assigned session branch.

## Bound

If the same finding returns three times after being fixed, stop (`AGENTS.md` §22). Do not merge. Report to the owner what is still open and why it keeps coming back.

## MUST NOT

- Merge with an unresolved human change request or a failing pre-commit hook.
- Add a hosted workflow. There is none.
- Commit or push directly to `main` unless the user prompt explicitly says to. The PR merge is the only way agent work reaches `main` ([task-workflow.md](./task-workflow.md)).
- Weaken a verifier, skip a test, or bypass the hook with `--no-verify` to end the loop ([local-verification.md](./local-verification.md)).
