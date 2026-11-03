# Agent Skills 1.0 Compliance

This template follows the **Agent Skills 1.0 specification** so any AI coding assistant can load the same skills, rules, and role definitions.

## Directory Structure

```
.agents/
├── agents/                  # Role prompts (Open Agent Format sub-agents)
│   └── filesquid-orchestrator.md
├── skills/                  # Reusable capabilities
│   └── gherkin-architect/   # BDD scenario generation
│       └── SKILL.md         # Skill instructions (YAML + Markdown)
├── rules/                   # Project-specific guardrails
│   └── sync-logic.md        # Documentation sync enforcement
├── scripts/                 # Executable scripts for agents
│   └── README.md            # Details on agent scripts
└── kb/                      # Synthesized AI knowledge base
    └── agent-skills-1.0.md  # Domain concepts for the AI

*Note: Raw technical specifications and whitepapers are kept in `docs/references/` rather than within `.agents/` to maintain a clear separation between project documentation and AI context.*
```

## Why `.agents/` instead of `.agent/`?

The `.agents/` directory is the **standard** defined by Agent Skills 1.0, ensuring:
- ✅ **Vendor neutrality** - Works across different AI coding assistants
- ✅ **Portability** - Skills can be shared between projects and tools
- ✅ **Discoverability** - AI assistants automatically detect `.agents/` directories
- ✅ **Future-proof** - Aligned with emerging industry standards

## Core Components

### 1. Skills (`.agents/skills/`)
**Reusable capabilities** that can be activated when needed.

**Example:** `gherkin-architect/SKILL.md`
- Translates SRS requirements into Gherkin scenarios
- Activated when user says "Draft Gherkin scenarios"
- Can be copied to other projects

### 2. Rules (`.agents/rules/`)
**Project-specific guardrails** that are always enforced.

**Example:** `sync-logic.md`
- Enforces consistency between PRD, SRS, Architecture, and Implementation Plan
- Always active - checked before any code is written
- Specific to this project's workflow

### 3. Knowledge Base (`.agents/kb/`)
**Synthesized domain knowledge** optimized for LLM conceptual understanding.
- Contains high-level summaries, established patterns, and project conventions.
- Loaded when the AI needs conceptual grounding (e.g., standard definitions, project vocabulary).

### 4. References (`docs/references/`)
**Raw technical specifications** for progressive disclosure.
- Contains deep API specs, protocol whitepapers, or large technical documents.
- Loaded *only* when a specific Skill explicitly requests it via its YAML frontmatter, saving tokens and improving accuracy for low-level tasks.

### 5. Scripts (`.agents/scripts/`)
**Executable scripts** that an AI agent can run.
- Used to validate logic, simulate load, or perform automated checks without deploying code.
- Often executed by specific Skills to verify architecture decisions or calculations.

### 6. Agents (`.agents/agents/`)
**Role prompts** for the workers that implement the roadmap. See [agents/README.md](./agents/README.md).

- A role is a sub-agent with its own session (orchestrator, modules, platform, UI).
- A skill is a procedure any of those workers loads when the task needs it.
- Role files follow the Open Agent Format simplified sub-agent definition: `name`, `description`, then the prompt. They name no coding tool and no model.

## Agent Skills 1.0 Format

Each skill follows this structure:

```markdown
---
name: Skill Name
description: Brief description of what this skill does
---

# Skill Name

## Purpose
[What this skill accomplishes]

## When to Use This Skill
[Triggers for activation]

## Instructions
[Detailed guidance for the AI agent]
```

## Portability

Skills, rules, and role files are Markdown and YAML. Any assistant that reads `.agents/` can load them. Distribution and how a session is started stay with the assistant. The files do not name a product.

## Migration from `.agent/`

If you have an existing project using `.agent/`, migrate to `.agents/`:

```bash
# Move skills
mv .agent/skills .agents/skills

# Move rules
mv .agent/rules .agents/rules

# Remove old directory
rmdir .agent
```

Then update all references in your documentation from `.agent/` to `.agents/`.

## Creating New Skills

1. Create a directory: `.agents/skills/my-skill/`
2. Add `SKILL.md` with YAML frontmatter
3. (Optional) Add `assets/` for templates or examples
4. Reference in documentation

**Example:**
```
.agents/skills/api-generator/
├── SKILL.md
└── assets/
    └── openapi-template.yaml
```

## Creating New Rules

1. Create a file: `.agents/rules/my-rule.md`
2. Write clear, enforceable guidelines
3. Reference in README or other docs

**Example:**
```markdown
# TypeScript-Only Rule

All new code MUST be written in TypeScript, not JavaScript.

## Enforcement
- Check file extensions (.ts, .tsx only)
- Reject .js files in pull requests
- Configure linter to enforce
```

## Benefits for AI-driven Development Projects

1. **Portability** - Use the same skills across different AI assistants
2. **Shareability** - Publish skills for the community
3. **Consistency** - Standard structure makes onboarding easier
4. **Vendor Independence** - Not locked into a single AI tool

## Learn More

- [Agent Skills 1.0 Specification](https://github.com/agent-skills/spec)
- [Example Skills Repository](https://github.com/agent-skills/examples)
