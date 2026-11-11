---
type: Project Standard
title: Agent Skills 1.0 Specification
description: Progressive disclosure, vendor neutrality, multi-tool support, and explicit grounding for
  AI agents in this repository.
tags:
- agents
- skills
- progressive-disclosure
- standards
status: stable
generated:
  by: process:kb-synthesizer
  at: '2026-07-26T18:00:00Z'
sources:
- id: agent-skills
  resource: .agents/kb/agent-skills-1.0.md
  title: Agent Skills 1.0 (project synthesis)
  author: process:project-template
kb-title: Agent Skills 1.0 Specification
last-updated: '2026-07-26T18:00:00Z'
domain: Agent Framework, Standards
---

# Agent Skills 1.0 Specification

**Purpose:** This document outlines the core principles of the Agent Skills 1.0 framework. It defines how AI agents should interact with tools, resources, and contextual knowledge to maximize efficiency and minimize hallucinations. AI agents working in this repository MUST follow these patterns.

## Core Principles

### 1. Progressive Disclosure (Context Optimization)
To avoid overwhelming the agent's context window (which leads to hallucinations and cognitive degradation), agents MUST NOT load all available information upfront. 
*   **Actionable Rule:** Agents must selectively load reference materials or technical specifications (e.g., from `/docs/kb/` or `/docs/references/`) *only* when the current task explicitly requires that deep domain knowledge.

### 2. Vendor Neutrality
Agent skills and instructions MUST NOT be hardcoded to rely on the proprietary quirks of a single AI model or coding assistant. 
*   **Actionable Rule:** Use standard industry formats (Markdown, YAML, JSON) for definitions. Ensure that prompt instructions are clear, logical, and universally understandable by any capable frontier model. Use standard standard terminology (like RFC 2119).

### 3. Multi-Tool Support
Agents are expected to combine multiple specialized tools dynamically to solve complex problems.
*   **Actionable Rule:** When a skill defines `allowed-tools` (e.g., `google_search`, `code_interpreter`, `file_system`), the agent MUST proactively use these tools to verify reality rather than relying on its pre-trained memory. (e.g., *Always* search the web to verify a library version; *always* run a script to validate a cost calculation).

### 4. Explicit Grounding (Knowledge Bases)
The repository acts as the primary source of truth. The agent's pre-trained knowledge is secondary.
*   **Actionable Rule:** Whenever a standard, terminology, or framework rule is defined in `/docs/kb/` or `AGENTS.md`, the agent MUST adopt that definition over its own internal biases. 

## Writing a Skill

Skills in this repository (located in `.agents/skills/` or `.agent/workflows/`) should follow this standard Markdown structure:

1.  **YAML Frontmatter:** Defining `name`, `description`, `allowed-tools`, and `references`.
2.  **Purpose:** What the skill does.
3.  **When to Use It:** Explicit triggers for the agent.
4.  **Workflow Steps:** Sequential pseudo-code (e.g., `AGENT: [Loads @docs...]`) guiding the agent's actions.
5.  **Success Criteria:** A verifiable checklist of what the skill must produce.
