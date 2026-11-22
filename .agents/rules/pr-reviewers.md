---
trigger: always_on
description: The external pull-request reviewers that are active, and each reviewer's account, signals and commands. Only the owner changes this file.
copyright: Pieter Van Garderen (2026)
license: CC-BY
---

# Pull Request Reviewers

**Purpose:** [pr-review-loop.md](./pr-review-loop.md) describes the review loop without naming a reviewer. This file is the one place that names them. To add, remove or replace a reviewer, the owner edits the table below, that reviewer's section, and its configuration. The loop, `AGENTS.md` and the role files do not change.

Only the owner edits this file. An agent that observes a reviewer behaving differently from its section reports that to the owner and does not edit the section.

## Active reviewers

| Reviewer | Active | Required before merge | Configuration |
|---|---|---|---|
| CodeRabbit | Since 2026-11-14. Its free trial ends 2026-11-28. | Yes | `.coderabbit.yaml` at the repository root |
| GitHub Copilot code review | Since 2026-11-22 | Yes | The repository ruleset "CoPilot Code Review" |

A reviewer marked "Required before merge" blocks the merge until it has reviewed the head commit and has no open finding. A reviewer marked "No" is advisory: answer its findings, but do not wait for it.

Owner decision, 2026-11-22, verbatim: "I changed my mind and want to run both CodeRabbit and Co-Pilot reviews on the same PRs side-by-side until the free trial ends upon which I will decide on the one I think works best on the Filesquid repo." Until the owner replaces the table, both reviewers are active on every pull request.

Reviewers' product names may appear in this file, in pull request text and in thread replies. The role files' ban on model and tool names covers coding tools and models, not reviewers.

## What reviewers are asked to look for

[`REVIEW.md`](../../REVIEW.md) at the repository root holds the review guidance, written for any reviewer. Both active reviewers read it from the pull request's branch without further configuration, together with `AGENTS.md`: CodeRabbit's published configuration schema lists `**/REVIEW.md` and `**/AGENTS.md` among the default patterns of `knowledge_base.code_guidelines.filePatterns`, and GitHub documents both file names for Copilot code review. A reviewer's own configuration holds only that reviewer's switches, never review guidance.

## Sections every reviewer has

Each reviewer's section answers the same questions, so the loop can refer to them by name:

- **Account:** the GitHub account its reviews and comments come from.
- **Starts:** what makes it review without being asked.
- **Reviewed the head commit:** what proves it reviewed the pull request's head commit.
- **Request a review:** how an agent asks for a review when none arrived.
- **Findings:** where its findings appear.
- **Never use:** actions agents MUST NOT take with it.
- **Limits:** what can delay or stop a review, and what to do.

## CodeRabbit

- **Account:** `coderabbitai[bot]`, a GitHub App.
- **Starts:** it reviews each new pull request and each push. It pauses automatic reviews after 5 reviewed commits on a pull request unless the branch's `.coderabbit.yaml` sets `reviews.auto_review.auto_pause_after_reviewed_commits` to `0`.
- **Reviewed the head commit:** any one of these.
  - A `coderabbitai[bot]` review with a non-empty body has `commit_id` equal to the head SHA; its body names the reviewed range ("Reviewing files that changed ... between `<old SHA>` and `<head SHA>`").
  - A clean review. CodeRabbit posts no review when it has nothing to say (observed 2026-11-22 on pull requests 188 and 191). Its summary comment, the `coderabbitai[bot]` issue comment that holds the walkthrough, then reads "No actionable comments were generated in the recent review", and that comment's `final_review_risk_coverage` marker names the head SHA as `coveredCommitId` with `kind` `reviewed`.
  - CodeRabbit answers `@coderabbitai review` with "Already reviewed the last commit", in a reply posted after the head commit was pushed.

  A `coderabbitai[bot]` review with an empty body only carries a reply on a thread and does not count. A completed `CodeRabbit` check run does not count by itself: it also completes, with `skipped` or `success`, when a review is paused or rate limited.
- **Request a review:** comment `@coderabbitai review` on the pull request, alone on its line. It "Reviews the commits pushed since CodeRabbit's last review of the pull request" ([CodeRabbit: Review commands](https://docs.coderabbit.ai/reference/review-commands)). `@coderabbitai full review` reviews every file again; post it only when the owner asks.
- **Findings:** inline review threads for actionable findings. Nitpick and outside-diff findings sit in the review body and have no thread. Replies on a thread can ask for more.
- **Never use:** `@coderabbitai resolve`, `@coderabbitai approve`, `@coderabbitai pause`, `@coderabbitai ignore`; `@coderabbitai review --use-credits` or the "Review with credits" box, because paid reviews are the owner's decision.
- **Limits:** reviews are metered per hour, and each review body states how many remain. When a review is rate limited CodeRabbit says so in a comment, and the check titled "Review rate limited" still passes. Comment `@coderabbitai rate limit` once to read when the next review is available, and request the review again after that time.

## GitHub Copilot code review

- **Account:** GitHub's Copilot reviewer. Its exact account name has not been observed in this repository yet (no Copilot review existed on 2026-11-22). Identify its reviews by the reviewer shown as Copilot on the pull request.
- **Starts:** the ruleset "CoPilot Code Review" requests it when a pull request is opened, draft pull requests included, and on every push. A pull request that was already open when the ruleset was created is reviewed from its next push, or when the owner requests a review.
- **Reviewed the head commit:** a review by the Copilot reviewer has `commit_id` equal to the head SHA. What Copilot posts when it finds nothing has not been observed in this repository yet.
- **Request a review:** in the pull request's "Reviewers" menu, select "Request" next to Copilot, or the re-request icon after a push ([GitHub: Using Copilot code review](https://docs.github.com/en/copilot/how-tos/use-copilot-agents/request-a-code-review/use-code-review)). An agent without that menu asks the owner to request it.
- **Findings:** inline review comments, which are ordinary resolvable threads, and a "Comment" review. It does not approve or request changes. A later review can repeat a comment that was already answered; answer it once more with a pointer to the earlier reply, and count it as a returning finding only when its substance was not addressed.
- **Never use:** nothing may be dismissed or hidden to end the loop. Agents MUST NOT change the ruleset or Copilot's settings.
- **Limits:** it does not review dependency manifests and lockfiles, log files or SVG files. Each automatic review is billed to the pull request author's Copilot allowance, so push a round's fixes together.
