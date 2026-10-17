---
type: Project Standard
title: Conventional Commits 1.0.0
description: Required Git commit message format (type, scope, description) for this project.
resource: https://www.conventionalcommits.org/en/v1.0.0/
tags:
- git
- commits
- conventional-commits
- standards
status: stable
generated:
  by: process:kb-synthesizer
  at: '2026-07-26T18:00:00Z'
sources:
- id: cc-spec
  resource: https://www.conventionalcommits.org/en/v1.0.0/
  title: Conventional Commits 1.0.0
kb-title: Conventional Commits 1.0.0
source-url: https://www.conventionalcommits.org/en/v1.0.0/
spec-version: 1.0.0
last-updated: '2026-07-26T18:00:00Z'
domain: Version Control, Standards
---

# Conventional Commits 1.0.0

**Purpose:** This document defines the exact formatting required for all Git commit messages in this project. It ensures a standardized, parsable, and clean history. All AI agents MUST format their commits according to this specification.

## Core Structure

A Conventional Commit MUST adhere to the following structure:

```
<type>[optional scope]: <description>

[optional body]

[optional footer(s)]
```

## Permitted Types

Agents MUST use one of the following types for every commit:

| Type | When to Use It |
| :--- | :--- |
| **`feat:`** | A new feature or requirement implementation (correlates with a MINOR release in semantic versioning). |
| **`fix:`** | A bug fix (correlates with a PATCH release in semantic versioning). |
| **`docs:`** | Documentation only changes (e.g., updating `README.md`, `AGENTS.md`, or the `/docs/kb/`). |
| **`style:`** | Changes that do not affect the meaning of the code (white-space, formatting, missing semi-colons, etc). |
| **`refactor:`** | A code change that neither fixes a bug nor adds a feature (e.g., renaming variables, extracting functions). |
| **`perf:`** | A code change that improves performance. |
| **`test:`** | Adding missing tests or correcting existing tests (including Gherkin `.feature` files). |
| **`build:`** | Changes that affect the build system or external dependencies. |
| **`chore:`** | Other changes that don't modify `src` or `test` files (e.g., updating `.gitignore`). |

## Formatting Rules

1.  **Description:** The `<description>` MUST immediately follow the colon and space after the type/scope. It MUST provide a short summary of the code changes.
2.  **Casing:** The `<type>` MUST be lowercase.
3.  **No Period:** The `<description>` MUST NOT end with a period.
4.  **Imperative Mood:** Use the imperative, present tense in the description (e.g., "change" not "changed" or "changes").

## Breaking Changes

If a commit introduces a breaking API change (correlates with a MAJOR release), it MUST be indicated by appending an exclamation mark `!` immediately before the colon (e.g., `feat!: change API response format`).

## Example Commits

*   `docs: update ISO 42010 compliance rule in AGENTS.md`
*   `feat: implement Gemini 3.1 Pro integration for intel fetching`
*   `test: add Gherkin scenario for live ticker monitoring`
*   `fix!: drop support for older SQLite schemas`
