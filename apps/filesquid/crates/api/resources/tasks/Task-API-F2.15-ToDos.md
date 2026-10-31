# Task API-F2.15 ToDos

- [ ] Failing domain tests (cause, home title, restore, retention, settings)
- [ ] One Recently deleted node, last, name null, has_children false
- [ ] View over the node: library ID and name, deleted_at, purge_after, cause, home title, folder; no folder rows; unreadable libraries excluded
- [ ] files/listing refuses the node; no library node of its own
- [ ] is:unfiled unknown; is:deleted finds both causes
- [ ] Delete collection: one version, times, cause, home kept
- [ ] Restore in place; untitled collection numbering; cap 9,999; restore_name_exhausted journaled failed
- [ ] Retention 1 to 3,650; default 30; library period in snapshot; survives rebuild; never moves purge_after; journaled
- [ ] Expiry setting default keep_version_history; admin only; journaled
- [ ] Expiry job: delete_all deletes object, index, record
- [ ] Expiry job: tombstone with facts, PROV-O records, purge record; OperationKind::Tombstone
- [ ] Crash at each tombstone step completed by recovery
- [ ] Recovered library shows tombstone as purged history, not a file
- [ ] Failure journaled failed and retried
- [ ] Redaction tests
- [ ] openapi.json 0.36.0 and schema.d.ts
- [ ] BDD features, SRS sync, walkthrough, plan and checklist boxes
