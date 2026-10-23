# Task 0.8 ToDos (documentation remainder)

Gates mirror the numbered constraints in [Task-0.8-Plan.md](./Task-0.8-Plan.md).

## Setup

- [x] Branch `docs/task-0.8-foundation-evidence` from `origin/main`; merge driver registered; `npm ci` in both UI packages
- [x] Search `resources/`, `docs/` and `apps/*/resources` for an existing register or budget document (none; extend nothing, create two)

## Capability register (`resources/capability-register.md`)

- [ ] 1 Columns: platform and prerequisite, outcome, requirement ID, evidence, F-040 state (equivalent / adapter equivalent / limited / preservation-only / unavailable) and the supported / limited / unsupported reading
- [ ] 2 Nonempty minimum supported set stated per committed conditional capability, or the row says the set is undeclared and pending
- [ ] 3 No aggregate row hides an unavailable child; every family row lists its children; no product comparison claimed
- [ ] 4 Fixture coverage rows (webviews, browsers, cameras/codecs, ICC/HDR, input methods, languages); F-041, F-044, F-045, F-046, F-047 child rows
- [ ] 5 Every unsupported row says what is preserved, viewable, editable, exportable and the safe alternative (one column, one line)
- [ ] 6 Performance-profile declarations recorded (in foundation evidence) with evidence "none"
- [ ] 7 Photos-baseline rows for F-002 to F-028 children; C-036, C-037, C-038, C-040 cited
- [ ] 8 No-star-ratings entry
- [ ] 9 Drive-parity table with all eleven differences and owning F-IDs
- [ ] 10 Internal names used; item type is file
- [ ] 11 Photo-manager parity register marked as ImageSquid's, pending
- [ ] 12 One row per F-001–F-067, NFR-001–010, TC-001–005; F-047, F-048 explicitly pending; F-050–F-060 against FileSquid phases; each row names the delivering task or PR

## Foundation evidence (`resources/foundation-evidence.md`)

- [ ] 13–19 NFR-002 budgets recorded as proposed with measured "none" and the measuring task; no U-11 claim
- [ ] 20–27 NFR-003 bounds recorded as proposed with measured "none"
- [ ] 28 RC-002 verification list and the 100-session / 20-transfer pilot recorded as a proposed test, not a capacity promise
- [ ] 29 Test-reporting table per workspace (runner, results path, cleanup, report command), citing UI-0.6 and the README
- [ ] 30 TC-003 platform and provider limits recorded as unverified pending their routes
- [ ] 31 TC-004 record: target architecture versus scaffold reality on `main` at the merge-base, labelled hypothetical, with the later-work list (T-034), Git-not-OCFL and SHA-256 statements
- [ ] `scripts/docs/validate-adrs.mjs` checked to read `resources/architecture/` (output recorded)

## README

- [ ] 32 View logs: journal, Monitor and OTLP mapped to FS-2.4 and FS-3.6; optional profile documented with the executed `docker compose --profile observability config --services`; About links the register

## Plan, checklist, walkthrough

- [ ] Task 0.8 row ticked and Status links the register, evidence and walkthrough; checklist box 19 ticked
- [ ] `python3 scripts/docs/update_mvp_progress.py` run; `--check` clean
- [ ] `resources/tasks/Task-0.8-Walkthrough.md` with SRS mapping (ID and full title) and raw outputs; Plan and ToDos deleted
- [ ] `bash scripts/pre-commit.sh` passes on the head (`Test scope: docs`)
- [ ] PR opened against `main`; Cubic loop finished; hand back
