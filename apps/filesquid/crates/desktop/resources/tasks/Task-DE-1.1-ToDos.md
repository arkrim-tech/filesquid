# Task DE-1.1 ToDos

- [ ] Plan constraint 3/7: `HostLocations` registers canonical folders under opaque IDs ≤ 128 bytes; `overlaps` true for unknown/nested; unit tests
- [ ] Constraint 8: one `HostLeases` per location shared by `HostFiles` and the library commands (`open_fs_library_root`); test
- [ ] Constraint 5: `SqliteRepairLedger` (consent durable first; outcome + receipt + journal row in one transaction); unit tests incl. survives reopen
- [ ] Constraint 1/2/9: desktop installs `LibraryCommands` via `install_libraries`; create/open/copy/rebuild through `filesquid://` integration tests
- [ ] Constraint 4: repair by the token holder only; no consent → refused, no change; integration test
- [ ] Constraint 6: refusals journaled once without path; integration test
- [ ] Constraint 3: no answer carries a path; integration test
- [ ] Constraint 10: history source lists owner-chosen roots; integration test
- [ ] Constraint 11: `FileService::install_provenance` at launch; test
- [ ] Constraint 12: navigation installed; webview subscription filtered; tests
- [ ] Constraint 13: admin backfill route (contract, action, tests, TS types)
- [ ] Constraint 15: folder choice per orchestrator decision
- [ ] Constraint 14: DE-S-015 source scan still passes
- [ ] Constraint 16: WebdriverIO spec for the packaged window
- [ ] Cucumber feature for SRS-F-001 desktop scenarios
- [ ] Walkthrough, plan/checklist boxes, progress bar, README, FULL=1 hook
