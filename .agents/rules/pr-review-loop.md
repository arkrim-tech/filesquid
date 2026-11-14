---
trigger: always_on
description: Run every agent pull request through the CodeRabbit review loop until no CodeRabbit thread or human change request remains open, then merge it into main.
copyright: Pieter Van Garderen (2026)
license: CC-BY
---

# Pull Request Review Loop (CodeRabbit)

**Purpose:** This rule enforces the PR Review Mandate in `AGENTS.md`. The external reviewer is CodeRabbit, a GitHub App installed on the repository (owner decision, 2026-11-14; it replaces the AI PR Agent workflow removed the same day). CodeRabbit is not a GitHub Actions workflow: it needs no workflow file and no repository secret, so the ban on hosted workflows in [local-verification.md](./local-verification.md) stands. Its review does not replace the local hook or a task's own tests. The owner has granted agents permission (2026-10-17) to merge their own pull requests into `main` once this loop is finished, for tasks inside the current milestone only.

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
2. **Poll for CodeRabbit.** CodeRabbit reviews each new pull request and each push on its own, and reports each review as a check run named `CodeRabbit` on the head commit. After each push, poll that check run with `gh api repos/<owner>/<repo>/commits/<sha>/check-runs` (REST), at most once every 5 minutes and at most 24 polls per round, from a background loop or a monitor tool, never a foreground `sleep`. At each poll also read the PR's `mergeable_state`; when it is `dirty`, merge `main` into the branch straight away. Agents MUST NOT subscribe to PR activity or create scheduled triggers, wake-ups or check-ins for the loop (owner decision, 2026-10-18); re-triggering CodeRabbit as below is allowed.
   - **A completed check run is not, by itself, a review.** CodeRabbit also completes the check run when it skips a review, is rate limited or stops on an error. The head commit counts as reviewed only when all of these hold:
     - The `CodeRabbit` check run on that commit is `completed` with conclusion `success`.
     - CodeRabbit's status on the pull request (its summary comment and review information) does not report the review as skipped, rate limited, paused or failed.
     - CodeRabbit does not list a changed file as left out of the review, other than a file the repository's `.coderabbit.yaml` excludes.
   - If the check run has not completed 15 minutes after the push or the last re-trigger, or it completed without a review as above, re-trigger CodeRabbit by commenting `@coderabbitai review` on the pull request. When CodeRabbit states a rate-limit wait, wait that long first. Re-trigger at most 3 times per head commit. The 24-poll cap covers the whole round, re-triggers included. If the re-triggers are used up or the cap is reached and the head commit still does not count as reviewed, stop, do not merge, and report the blocker to the owner.
   - Agents MUST NOT comment `@coderabbitai resolve`, `@coderabbitai approve`, `@coderabbitai pause` or `@coderabbitai ignore`, and MUST NOT change `.coderabbit.yaml` to narrow what is reviewed. These close threads or switch the reviewer off without a fix; only the owner uses them.
3. **Fix every CodeRabbit comment and every human review comment that requests a change.** For each open CodeRabbit or human review thread:
   - Fix it in code or documentation, following every other rule in `.agents/rules/` (TDD, lint, docs, links).
   - If the finding is wrong, reply on the thread with the reasoning and evidence, then resolve the thread. Never dismiss a finding silently.
   - Each thread gets exactly one reply: the fix commit or the reasoning.
   - If you disagree with a security finding, leave the thread open and ask the owner.
   - A CodeRabbit finding that arrives outside a thread (in its summary comment, or as a failed pre-merge check) is answered the same way, in one pull-request comment that names the finding and the fix commit or the reasoning.
4. **Push the fixes.** Rerun `bash scripts/pre-commit.sh` (its change-aware scope is enough for a fix commit), commit with Conventional Commits, and push to the same branch. Reply on each thread you fixed, naming the commit, and resolve it. Resolve the thread with the host's review API. When GraphQL is available, use the `resolveReviewThread` mutation.
5. **Repeat** steps 2 to 4 until the merge conditions below hold.

## Merge conditions

Merge the PR into `main` only when all of these are true:

- CodeRabbit has reviewed the current head commit, as step 2 defines "reviewed". A clean pass may post no new thread, so the check run together with CodeRabbit's status is the signal; new findings arrive as review threads.
- No CodeRabbit review thread is unresolved.
- The hook passed on the current head commit with the run its scope requires: `FULL=1 bash scripts/pre-commit.sh`, summary line `Test scope: full`, when the PR's change scope (`scripts/changed_scope.sh` against `origin/main`) is `full` or includes `rust:`; otherwise a passing ordinary `bash scripts/pre-commit.sh`, summary line `Test scope: <scope>`.
- The PR has no merge conflict with `main`. Resolve a conflict by merging `main` into the branch, then return to step 2.
- No human reviewer has requested changes that are still open.

Use a merge commit (the repository's existing convention), then delete the task branch on the remote if it is not the agent's assigned session branch.

## Bound

The loop is capped at **10 CodeRabbit review rounds** per PR (`AGENTS.md` §22). If CodeRabbit still has open threads after round 10, or the same finding returns three times after being fixed, stop. Do not merge. Report to the owner what is still open and why it keeps coming back.

## MUST NOT

- Merge with an unresolved CodeRabbit thread, an unresolved human change request, a failing pre-commit hook, or before CodeRabbit has reviewed the head commit.
- Close CodeRabbit threads or switch CodeRabbit off with its own commands (`resolve`, `approve`, `pause`, `ignore`) or by editing `.coderabbit.yaml`.
- Add a hosted workflow. There is none, and CodeRabbit needs none.
- Commit or push directly to `main` unless the user prompt explicitly says to. The PR merge is the only way agent work reaches `main` ([task-workflow.md](./task-workflow.md)).
- Weaken a verifier, skip a test, or bypass the hook with `--no-verify` to end the loop ([local-verification.md](./local-verification.md)).
