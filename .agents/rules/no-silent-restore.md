---
copyright: Pieter Van Garderen (2026)
license: CC-BY
---

# No Silent Restore of User File Changes

**Purpose:** Prevent AI agents from undoing intentional human deletes or revisions by automatically restoring prior content.

## Mandate

When any of the following occur, the agent MUST treat the change as potentially intentional:

- The user deletes a file or directory
- The user revises a file (including emptying it or replacing large sections)
- Git status / workspace inspection shows a deletion or unexpected content change that the agent did not make in the current turn

## Required behavior

1. **Do not restore** previous content, re-create deleted paths, or `git checkout` / `git restore` user-touched files solely because the change “looks accidental.”
2. **Ask first:** Prompt the user whether the deletion or revision was intentional and should stick.
3. **Restore only after explicit confirmation** from the user (e.g. “yes, restore it” / “that was a mistake”).
4. **Agent’s own mess is different:** The agent MAY and SHOULD clean up or revert files **it** created or broke in the same task without asking, when that cleanup is part of fixing its own incomplete work—provided it does not overwrite deliberate user edits made in parallel.

## Examples

| Situation | Correct action |
|-----------|----------------|
| User deletes `docs/assets/diagram.png` | Leave it deleted; ask only if about to re-add it for another reason, or if unclear |
| Agent accidentally deletes a file mid-edit | Agent may restore **its own** accidental delete without a full confirmation ritual |
| User empties a config file on purpose | Do not restore old contents; ask if next steps require that file |

## Enforcement

This rule overrides default “helpful recovery” behavior. It applies to every agent operating in this repository that loads `AGENTS.md` and `.agents/rules/`.
