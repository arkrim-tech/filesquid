# Review guidance

This file tells any pull-request reviewer, human or automated, what to look for in FileSquid. It names no reviewer and holds no reviewer's settings. The reviewers that are active are listed in [pr-reviewers.md](./.agents/rules/pr-reviewers.md).

The binding project rules are [AGENTS.md](./AGENTS.md) and every file in [`.agents/rules/`](./.agents/rules/). Review each change against them, read from the pull request's own branch. The sections below point at the defects that are easy to miss when only the diff is read.

## Every change

- Follow each changed call into the unchanged code it reaches before concluding. Report a defect that only shows across functions or files.
- Hiding or removing something that existed before is a behaviour change. Say which user action or check is no longer reachable.
- Some pull requests are never merged by an agent: the list is the "Never self-merge" entry of the orchestrator's [merge procedure](./.agents/agents/filesquid-orchestrator.md#merge-procedure). This file and each reviewer's configuration belong to the owner in the same way ([pr-reviewers.md](./.agents/rules/pr-reviewers.md)). When a pull request touches one of those, say that it is for the owner to merge.
- A pull request MUST NOT add a file under `.github/workflows/` or a `.gitlab-ci.yml`.

## Rust crates: `apps/filesquid/crates/**/src/**`

Review against `AGENTS.md` sections 13, 15 and 21 to 26.

- Every error is logged, returned or raised. Flag a discarded `Result`, a `let _ =` or `.ok()` that drops an error, and a function that reports success after an inner call failed.
- Every loop, poll and retry has a cap and a safe path when the cap is hit. Flag a tight loop that can log or write once per tick for ever.
- Every resource opened is closed on every exit path, the error path included. No lock or database connection is held across an await.
- One function does one job and is at most 60 lines. A function that does I/O or mutates state says so in its name. Flag getter-named functions that do I/O.
- Every API route is authorised before it touches files, SQL or keys.
- A module crate takes no default dependency on another module crate.
- Bounds or constants marked provisional must not be frozen into a database schema or migration.
- Public items carry rustdoc. Logs go to stdout only, never to a file.

## UI packages: `apps/filesquid/ui/src/**` and `apps/imagesquid/src/**`

Review against `AGENTS.md` sections 15 to 17.

- Vanilla TypeScript custom elements only: no UI framework and no runtime npm dependency.
- The UI reaches data only through the FileSquid API client, never the filesystem, a database or a module directly.
- Listeners, observers, timers and abort controllers are released in `disconnectedCallback` and on every replace or error path. Late `load`, `error` or promise results must not update a newer state.
- A client-side gate (hide, disable, redirect) is cosmetic. Check that the API enforces the same rule and that the gate cannot lock a permitted user out. Follow error values into the client code that parses them.
- Text comes from the label tables, with every new key present in both `apps/filesquid/ui/src/theme/labels.en.json` and `apps/imagesquid/src/theme/labels.en.json`. No hard-coded English.
- Names and other data are set with `textContent`, never `innerHTML`.
- Stylesheets use logical properties so right-to-left layouts mirror, and a `display` rule must not defeat the `hidden` attribute.
- Exported items carry TSDoc.

## Tests: `apps/**/tests/**`

Judge whether each test would fail if the feature were broken.

- Flag assertions that restate the implementation, tests that only exercise a stub written to mirror the code, and tests that inject a state no user path can produce.
- Flag production wiring that no test executes, for example a spawn, a route binding or an entry module that no test imports.
- Flag error, refusal and failure paths that the change adds but no test covers.
- UI behaviour must be asserted against the built production artifact with DOM assertions, not only in a simulated DOM that applies no stylesheet.
- Flag fixed sleeps and timing assumptions.

## Walkthroughs: `**/resources/tasks/*Walkthrough.md`

Check the walkthrough's claims against the diff in the same pull request.

- It holds the raw output of the test runs it reports, and a coverage report when a test suite ran. A docs-only run that skipped the suites needs neither.
- The pre-commit summary line `Test scope: <scope>` for the head commit is in the walkthrough or in the pull request description. Either place is enough.
- A suite that was not run is named as not run. A failing run is not described as passing, or waved through as pre-existing without evidence.
- Every "tested" or "covered" claim has a matching test in the diff.
- Every documented command appears to have been executed in the environment the walkthrough shows.
- Tests were written before the implementation. Flag a statement that they were not.
- No absolute machine path appears.

## Gate scripts: `scripts/**`

These scripts are the project's quality gates.

- A change must not weaken or narrow a gate. State exactly what the gate accepted before and accepts after.
- A gate must fail closed. Flag an error hidden by `2>/dev/null`, a missing tool that leads to a pass, and a pipeline whose failure is swallowed.
- Scripts run on bash 3.2 with BSD and GNU userland. Flag `sed` escapes such as a tab written as backslash-t in a replacement, `readlink -f`, `mapfile` and associative arrays.
- Paths with spaces or special characters must survive quoting and parsing.

## API contract: `apps/filesquid/crates/api/openapi.json`

The contract, the Rust types, `apps/filesquid/ui/src/api/schema.d.ts` and `apps/filesquid/ui/src/api/contract-version.ts` must agree, and a breaking change must raise the contract version.
