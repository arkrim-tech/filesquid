---
description: Role prompts live only in .agents/agents. Do not copy or symlink them into a tool directory.
copyright: Pieter Van Garderen (2026)
license: CC-BY
---

# Agent role location

**Purpose:** Keep one copy of the FileSquid role prompts, and stop agents from recreating the tool-specific symlink directories that were removed on 2026-11-22.

## Mandate

The only role prompts are the Open Agent Format files in [`.agents/agents/`](../agents/README.md):

- [filesquid-orchestrator.md](../agents/filesquid-orchestrator.md)
- [filesquid-modules-engineer.md](../agents/filesquid-modules-engineer.md)
- [filesquid-platform-engineer.md](../agents/filesquid-platform-engineer.md)
- [filesquid-ui-engineer.md](../agents/filesquid-ui-engineer.md)

Read a role from that directory. When a session needs a role, open `.agents/agents/<role>.md` or paste that file into the session. Do not search another path first.

## Do not recreate tool copies

Do not create, restore, symlink, or look for role prompts under:

- `.grok/agents/`
- `.claude/agents/`
- any other tool-specific agents directory in this repository (`.tool/agents/`)

A missing `.grok/` or `.claude/agents/` directory is intentional. Do not treat it as drift, do not recover the files from git history, and do not add a symlink so a harness can auto-discover the roles.

If a coding tool only auto-loads roles from its own directory, leave that discovery empty. Point the session at `.agents/agents/` instead.

## What stays

- Skills stay in [`.agents/skills/`](../skills/). Rules stay in `.agents/rules/`.
- Historical walkthroughs may still mention `.claude/agents/`. Leave those mentions. Do not recreate the directory to match them.
- `scripts/changed_scope.sh` still classifies a path under the root `.claude/` as docs scope. That classification is not a request to put files there.
