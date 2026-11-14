---
trigger: always_on
description: Run every agent pull request through the CodeRabbit review loop until CodeRabbit has no further comments, then merge it into main.
copyright: Pieter Van Garderen (2026)
license: CC-BY
---

# Pull Request Review Loop (CodeRabbit)

**Purpose:** This rule enforces the PR Review Mandate in `AGENTS.md`. The external reviewer is CodeRabbit, a GitHub App that posts as `coderabbitai[bot]` (owner decision, 2026-11-14, below). It is installed on the repository and runs from no workflow file, so there is still no hosted workflow ([local-verification.md](./local-verification.md)). The owner has granted agents permission (2026-10-17) to merge their own pull requests into `main` once this loop is finished, for tasks inside the current milestone only.

## Owner decision, 2026-11-14

Verbatim: "We're abadoning PR Agent for code reviews and switching to Code Rabbit. Code Rabbit is now configured for all new PRs. I will follow-up manually to ensure the existing PRs also get a CodeRabbit review first. Your own rules and instructions should include waiting for a PR code review comments from the Agent and fixing any issues or comments as many times as necessary until the Agent has no further comments and the PR is read for human review to merge."

- It amends the removal of the AI PR Agent workflow earlier the same day: there is again an external reviewer, and agents wait for it.
- Pull requests opened before 2026-11-14 wait for the owner to start their CodeRabbit review. Do not request a CodeRabbit review on them, and do not merge them, until CodeRabbit has reviewed them once; then run the loop below.
- The reviewer's product name, CodeRabbit, may appear in rules, pull request text and thread replies. The role files' ban on model and tool names covers coding tools and models, not the reviewer.

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
2. **Wait for CodeRabbit's review of the head commit.** CodeRabbit reviews every new pull request on its own and reviews again when commits are pushed ([CodeRabbit: Manage code reviews](https://docs.coderabbit.ai/guides/commands)). After each push, poll the PR over REST:
   - `gh api --paginate repos/<owner>/<repo>/pulls/<n>/reviews`: reviews by `coderabbitai[bot]`. Each names the commit it reviewed in `commit_id`, and its body carries the nitpick and outside-diff comments.
   - `gh api --paginate repos/<owner>/<repo>/pulls/<n>/comments`: CodeRabbit's inline comments and its replies on threads.
   - `gh api --paginate repos/<owner>/<repo>/issues/<n>/comments`: CodeRabbit's summary comment and status notes.

   These list endpoints return 30 items per page by default, so always pass `--paginate`. They do not show whether a review thread is resolved: read each thread's `isResolved` from the paginated GraphQL `reviewThreads` of the pull request when GraphQL is available, or from the thread listing the agent's environment provides where GraphQL is refused. A reply on a thread is not proof that it is resolved.
   - `gh api repos/<owner>/<repo>/pulls/<n>`: `mergeable_state`. When it is `dirty`, merge `main` into the branch straight away; that is a push, so the round starts again.

   Poll at most once every 5 minutes and at most 24 polls per round, from a background loop or a monitor tool, never a foreground `sleep`. Agents MUST NOT subscribe to PR activity or create scheduled triggers, wake-ups or check-ins for the loop (owner decision, 2026-10-18).
   - CodeRabbit has reviewed the head commit when a `coderabbitai[bot]` review with a non-empty body has `commit_id` equal to the head SHA; its body names the reviewed range ("Reviewing files that changed ... between `<old SHA>` and `<head SHA>`"). A `coderabbitai[bot]` review with an empty body is only the carrier of a CodeRabbit reply on a thread, and does not count, even when its `commit_id` is the head SHA.
   - If it has not done so 15 minutes after the push, comment `@coderabbitai review` on the PR, alone on its line, at most once per head commit. That command "Reviews the commits pushed since CodeRabbit's last review of the pull request" ([CodeRabbit: Review commands](https://docs.coderabbit.ai/reference/review-commands)). `@coderabbitai full review` "Reviews every file in the pull request again from scratch" (same page); post it only when the owner asks. Post neither on a PR opened before 2026-11-14 that CodeRabbit has not yet reviewed.
3. **Fix every CodeRabbit comment.** Every finding counts: actionable inline comments, nitpicks and outside-diff comments (the last two sit in the review body, not in threads), and any CodeRabbit reply on a thread that asks for more. For each:
   - Fix it in code or documentation, following every other rule in `.agents/rules/` (TDD, lint, docs, links).
   - If the finding is wrong, answer it with the reasoning and evidence. Never dismiss a finding silently.
   - Each thread gets exactly one reply. For a finding you fix, that reply names the fix commit and is posted after the push in step 4. For a finding you answer without a fix, it is the reasoning, posted now. Resolve the thread with the host's review API, only after its reply. When GraphQL is available, use the `resolveReviewThread` mutation. GitHub documents no REST route for this, so where GraphQL is refused use the thread-resolution route or tool the agent's environment provides; if it provides none, ask the owner to resolve the thread in the GitHub UI. If the reviewer answers that reply with a new request or question, that is a new finding and gets one reply of its own.
   - Findings with no thread (nitpick and outside-diff comments) get one PR comment per review that answers each of them by name, with the fix commit or the reasoning. Post it once that round's fixes are pushed, or immediately when the round has no fixes to push.
   - If you disagree with a security finding, leave its thread open and ask the owner.

   Human review comments are handled the same way.
4. **Push the fixes.** Rerun `bash scripts/pre-commit.sh` (its change-aware scope is enough for a fix commit; the merge conditions below still require the scope's head-commit run, `FULL=1` for a `full` or `rust:` scope, before merge), commit with Conventional Commits, and push to the same branch. Push a round's fixes together: CodeRabbit reviews each push, and its review allowance per hour is limited (each review body states how many remain). Then reply on each thread you fixed, naming the commit, and resolve it.
5. **Repeat** steps 2 to 4, as many rounds as needed, until CodeRabbit's latest review is of the head commit, it has no further comments, and no CodeRabbit thread is unresolved. The PR is then ready for human review to merge.

CodeRabbit **has no further comments** when no finding from its review of the head commit, or from its replies since, is still open. A finding is closed when it was answered with reasoning in one reply (or, for a finding with no thread, in the PR comment of step 3) and its thread, if any, is resolved. A finding fixed in code needs a new commit, so a new head and a new CodeRabbit review. A finding answered without a code change needs no further commit or review.

## Merge conditions

Merge the PR into `main` only when all of these are true:

- CodeRabbit's latest review is of the head commit, it has no further comments, and no CodeRabbit thread is unresolved. A security thread left open for the owner blocks the merge until the owner settles it.
- The hook passed on the current head commit with the run its scope requires: `FULL=1 bash scripts/pre-commit.sh`, summary line `Test scope: full`, when the PR's change scope (`scripts/changed_scope.sh` against `origin/main`) is `full` or includes `rust:`; otherwise a passing ordinary `bash scripts/pre-commit.sh`, summary line `Test scope: <scope>`.
- The PR has no merge conflict with `main`. Resolve a conflict by merging `main` into the branch, then return to step 2.
- No human reviewer has requested changes that are still open.

These conditions hold whoever merges. Whether an agent may merge, or leaves the PR for the owner, follows the permission in the Purpose and the "Current milestone" section above.

Use a merge commit (the repository's existing convention), then delete the task branch on the remote if it is not the agent's assigned session branch.

## Bound

The loop has no fixed round limit; it runs until CodeRabbit has no further comments. It stops early (`AGENTS.md` §22) in these cases. Do not merge; report to the owner what is still open and why.

- The same finding returns three times after being fixed.
- CodeRabbit has not reviewed the head commit 60 minutes after a push, the one `@coderabbitai review` request included. This does not apply to a PR opened before 2026-11-14 that is still waiting for the owner to start its first CodeRabbit review; report that PR as waiting.
- The 24-poll cap of a round is reached without a CodeRabbit review of the head commit.

## MUST NOT

- Merge with an unanswered CodeRabbit comment, an unresolved CodeRabbit or human thread, a failing pre-commit hook, or before CodeRabbit has reviewed the head commit.
- End the loop with `@coderabbitai resolve`, `@coderabbitai approve`, `@coderabbitai pause` or `@coderabbitai ignore`. Each finding gets its own answer.
- Add a hosted workflow, or add or change CodeRabbit's configuration unless the owner asks. CodeRabbit is a GitHub App and needs no workflow file.
- Commit or push directly to `main` unless the user prompt explicitly says to. The PR merge is the only way agent work reaches `main` ([task-workflow.md](./task-workflow.md)).
- Weaken a verifier, skip a test, or bypass the hook with `--no-verify` to end the loop ([local-verification.md](./local-verification.md)).
