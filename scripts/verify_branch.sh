#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2026 Peter Van Garderen
#
# SPDX-License-Identifier: MIT
#
# Branch validation: prevents direct commits to main/master to enforce the Branch-Per-Task workflow.

BRANCH_NAME=$(git branch --show-current)
GIT_DIR=$(git rev-parse --git-dir 2>/dev/null)

# During an active rebase HEAD is detached; validate the branch being rebased instead.
REBASE_HEAD_FILE=""
[ -f "$GIT_DIR/rebase-merge/head-name" ] && REBASE_HEAD_FILE="$GIT_DIR/rebase-merge/head-name"
[ -f "$GIT_DIR/rebase-apply/head-name" ] && REBASE_HEAD_FILE="$GIT_DIR/rebase-apply/head-name"
if [ -z "$BRANCH_NAME" ] && [ -n "$REBASE_HEAD_FILE" ]; then
  BRANCH_NAME=$(sed 's#^refs/heads/##' "$REBASE_HEAD_FILE")
  echo "ℹ️  Active rebase detected; validating the rebased branch '$BRANCH_NAME'."
fi

if [ -z "$BRANCH_NAME" ]; then
  echo "❌ Branch Validation Failed: Detached HEAD state detected outside of a rebase of a named branch."
  echo "   Committing directly in detached HEAD bypasses branch protections."
  exit 1
fi

if [ "$BRANCH_NAME" = "main" ] || [ "$BRANCH_NAME" = "master" ]; then
  if [ "$FORCE_MAIN_COMMIT" = "1" ]; then
    echo "⚠️  WARNING: Direct commit to '$BRANCH_NAME' forced by human override."
    exit 0
  fi
  echo "❌ Branch Validation Failed: You are attempting to commit directly to the '$BRANCH_NAME' branch."
  echo "   Please adhere to the Branch-per-Task workflow. Create a feature branch (e.g., 'git checkout -b feat/name') to commit these changes."
  echo "   (Human override: prefix your command with FORCE_MAIN_COMMIT=1)"
  exit 1
fi

# Branch names MUST be <type>/<slug>, where <type> is a permitted Conventional Commit type
# (.agents/kb/conventional-commits.md). Prefixes such as task/ or claude/ are rejected.
PERMITTED_PREFIX_PATTERN='^(feat|fix|docs|style|refactor|perf|test|build|chore)/[a-z0-9][a-z0-9._-]*$'
if ! [[ "$BRANCH_NAME" =~ $PERMITTED_PREFIX_PATTERN ]]; then
  echo "❌ Branch Validation Failed: '$BRANCH_NAME' does not start with a permitted Conventional Commit type."
  echo "   Use <type>/<lowercase-slug> with type feat, fix, docs, style, refactor, perf, test, build or chore"
  echo "   (for example feat/io-0.1-storage-backend). task/ and claude/ are not allowed."
  echo "   Rename it with: git branch -m <type>/<slug>"
  exit 1
fi

echo "✅ Branch Validation passed."
exit 0
