# Task DE-1.8 ToDos

- [ ] A. `SqliteJobItems` and install on `Jobs`
- [ ] A. Job records queued/copying/copied/skipped/failed with code; bytes progress
- [ ] A. Retry pass imports only retried files; a run with a failed file stays pending; replay skips a job waiting for a retry
- [ ] B. `SqliteImportDigests`; API answers the committed digest
- [ ] B. `plan` after the dialog with no byte staged (facts, items, fingerprint)
- [ ] B. `start_plan` only after confirmation; `source_changed`; discard; replay
- [ ] B. Present files skipped
- [ ] C. `RebuildRunner::with_stop`; suspend at exit with a bounded wait
- [ ] C. Unfinished rebuild handed to the runner at launch (not paused ones)
- [ ] D. `SqliteLibraryRegistry`; reload at launch; recent libraries and free bytes after restart, no path
- [ ] Windows: free bytes `volume_unreadable` (unchanged behaviour, documented)
- [ ] BDD scenarios
- [ ] Docs, README commands, plan and checklist ticks, progress block
- [ ] fmt, clippy, tests; pre-commit; walkthrough; delete Plan and ToDos
