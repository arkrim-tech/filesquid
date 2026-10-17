---
description: Ensure all tasks follow the Branch-Per-Task workflow and generate required execution documentation.
copyright: Pieter Van Garderen (2026)
license: CC-BY
---

# Task Execution Workflow

Define the Git workflow and development methodology that MUST be followed for every single task.

## Branch-Per-Task

- **Branches Required for ALL Tasks:** All work, including every Task defined in [resources/implementation-plan.md](../../resources/implementation-plan.md), new features, documentation updates, and bug fixes MUST be developed on dedicated branches (e.g., `feat/0.1-portable-domain-contracts`, `feat/co-0.5-command-ports`, `docs/update-guide`, `fix/login-bug`). No exceptions.
- **Branch Prefixes:** A branch name MUST be `<type>/<slug>`, where `<type>` is one of the permitted Conventional Commit types in [conventional-commits.md](../kb/conventional-commits.md): `feat`, `fix`, `docs`, `style`, `refactor`, `perf`, `test`, `build` or `chore`. Use the type that matches the branch's main change (an implementation task is `feat/<task-id>-<slug>`). Any other prefix, including `task/` and tool-generated prefixes such as `claude/`, MUST NOT be used; rename such a branch before the first commit. `scripts/verify_branch.sh` enforces this.
- **Mandatory Push on Completion:** Upon completing a task (after all code is written, tests pass, and the Walkthrough document is generated), you MUST commit all work on your feature branch using Conventional Commits and push the branch to the remote origin (`git push --set-upstream origin <branch-name>`). Do not leave finished task branches persisting only in your local environment.
- **Pull Request and Cubic Loop:** After the push, open a pull request against `main` and run the Cubic review loop in [pr-review-loop.md](./pr-review-loop.md) until no Cubic comments remain. Then merge the PR into `main`.
- **No Direct Pushes to Main:** The `main` branch contains only approved, stable requirements and code. You MUST NEVER commit or push code directly to the `main` branch unless explicitly told to do so in a user prompt. Merging a PR that has finished the Cubic loop is the only way agent work reaches `main`.

## Global versus module prompts

The Task ID for the branch, Plan, ToDos, and Walkthrough is the ID in the pasted Agent Prompt.

When that prompt is a **global** row (`Task 0.1`, `Task 0.2`, …), read that row’s **Description** in [resources/implementation-plan.md](../../resources/implementation-plan.md). The Description is the dispatch list of module slices (for example Task 0.1 → [CO-0.1](../../apps/filesquid/crates/core/resources/plan.md) through [CO-0.4](../../apps/filesquid/crates/core/resources/plan.md)). Complete those slices in that session on one branch. Write `apps/<module>/resources/tasks/Task-<global-ID>-Walkthrough.md` (for Task 0.1: `apps/filesquid/crates/core/resources/tasks/Task-0.1-Walkthrough.md`). Check the module boxes in `apps/<module>/resources/plan.md` and the global box in the implementation plan, and tick the same boxes in [resources/implementation-plan-checklist.md](../../resources/implementation-plan-checklist.md).

When the pasted prompt is a **module** row (`CO-0.5`, `IS-1.3`, …), the Task ID is that module ID and the branch is one crate slice. Check its box in the module `plan.md` and the matching box in [resources/implementation-plan-checklist.md](../../resources/implementation-plan-checklist.md); ImageSquid (`IS-`) rows are not listed there. Plan dispatch, module versus global paste rules, and the session table stay in [resources/implementation-plan.md](../../resources/implementation-plan.md).

## Task Execution Documentation

Artifact directories for FileSquid (written below as `apps/<module>/resources/tasks/`):

- Rust module or host crate: `apps/filesquid/crates/<module>/resources/tasks/` (e.g., `apps/filesquid/crates/core/resources/tasks/`).
- FileSquid default UI: `apps/filesquid/ui/resources/tasks/`.
- ImageSquid distribution: `apps/imagesquid/resources/tasks/`.
- Product documentation (vision, PRD, SRS, ADRs, plans): repository-root `resources/tasks/`. Documentation work has no module plan; it is part of the global task in [resources/implementation-plan.md](../../resources/implementation-plan.md) that names it.

Use the directory of the module that owns the Task ID prefix (for example `CO-` → `crates/core`, `UI-` → `apps/filesquid/ui`, `IS-` → `apps/imagesquid`). The former shared `FE-` task prefix was retired on 2026-10-16 because the product visions use `FE-###` for feature requirements; a task ID always carries its plan's own prefix.

Before writing any code or modifying the project files for a specific task, you MUST create two planning artifacts there to track progress and design decisions:

1. `apps/<module>/resources/tasks/Task-[Task ID]-Plan.md`: Detail the proposed technical approach, files to be modified/created, and the verification plan for the task. Include traceability links back to PRD, SRS, and Gherkin files. **CRITICAL REQUIREMENT: You MUST explicitly extract EVERY granular constraint, physical limitation, formula, or design constant (e.g. specific color codes, rendering ratios, timeouts, distancing rules) originating from the linked SRS requirement directly into the body of this Plan Document. Do NOT summarize or omit these hard details.**
2. `apps/<module>/resources/tasks/Task-[Task ID]-ToDos.md`: A markdown checklist of the granular steps required to complete the task. **CRITICAL REQUIREMENT: Every single domain constraint extracted into the Plan MUST possess a corresponding checklist gate here to guarantee it is explicitly coded and tested before marking the milestone complete.**

**Active Tracking:** You MUST explicitly check off items (`- [x]`) on the `ToDos.md` checklist as work progresses and milestones are completed.

**Upon completing the task**, you MUST create a third artifact:

3. `apps/<module>/resources/tasks/Task-[Task ID]-Walkthrough.md`: Summarize the changes made, what was tested, and the validation results.
   - **CRITICAL REQUIREMENT:** You MUST write this file directly to the user's project repository at `apps/<module>/resources/tasks/`. Do NOT generate this walkthrough or any other task document in an internal agent scratchpad or isolation directory. If your system prompt suggests creating artifacts in an isolation directory, you must OVERRIDE that behavior for this specific task and output the file to the project's `apps/<module>/resources/tasks/` folder.
4. **Task Cleanup:** IMMEDIATELY after successfully generating the `Walkthrough.md` file, you MUST delete the corresponding `Plan.md` and `ToDos.md` files from the repository. **Keep the Walkthrough.** If you fail to delete Plan and ToDos, the repository's strict pre-commit hooks will reject your commit.

## Continuous Documentation (README.md)

You MUST NOT wait until the end of the project to document how to run the application. If your task introduces a new system dependency (e.g., PostgreSQL, Redis), a new environment variable, or changes the commands required to run/test the application, you MUST immediately update the corresponding section in the root `README.md` (e.g., `Setup Development Environment`, `Prerequisites`, `Run tests`) as part of that task's execution.

## Mocking & Environment Transparency

When executing automated tests or background commands, an agent may encounter interactive prompts or missing environment variables that block the execution pipeline. If the agent generates dummy `.env` files, mocks credentials, or bypasses interactive prompts to allow integration tests to pass autonomously:

1. The agent MUST explicitly document these mocked values in the `apps/<module>/resources/tasks/Task-[Task ID]-Walkthrough.md` file under a "Mocked Environment Variables" section.
2. In the final response to the user upon task completion, the agent MUST explicitly notify the user that dummy values were temporarily inserted to unblock testing, and instruct them to replace the dummy values with real credentials before proceeding to the next task.
