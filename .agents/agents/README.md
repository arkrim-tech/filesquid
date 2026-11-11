# FileSquid agent roles

These files are [Open Agent Format](https://openagentformat.com/spec.html) simplified sub-agent definitions. Each one is YAML frontmatter (`name`, `description`) followed by a Markdown role prompt. A role is a worker with its own session. It is not an Agent Skill. Skills stay in [../skills/](../skills/) and are loaded into whichever worker needs that procedure.

[Open Agent Specification](https://oracle.github.io/agent-spec/) (Agent Spec) describes an agent as a component in a declared flow. These roles are open-ended coding sessions, so the portable form is the sub-agent file. A second Agent Spec graph would only repeat the same prompt.

The orchestrator coordinates. It does not implement. Give each engineer one module task id per run.

| Role | File | Task prefixes |
|---|---|---|
| Orchestrator | [filesquid-orchestrator.md](./filesquid-orchestrator.md) | Dispatches the rows below |
| Modules engineer | [filesquid-modules-engineer.md](./filesquid-modules-engineer.md) | `CO-`, `IO-`, `MD-`, `CP-` |
| Platform engineer | [filesquid-platform-engineer.md](./filesquid-platform-engineer.md) | `API-`, `CL-`, `SV-`, `BE-`, `DE-` |
| UI engineer | [filesquid-ui-engineer.md](./filesquid-ui-engineer.md) | `UI-`, `IS-` |

A harness that supports sub-agents loads the matching file as that worker's prompt. A harness that does not can still paste the file into a fresh session. The prompt names no coding tool and no model.
