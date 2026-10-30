# Task API-F2.14 ToDos

- [ ] Constraint 1: upload and copy inputs take `collection_id` and optional `folder` (reserved folders refused)
- [ ] Constraint 2: `destination_required` before any byte is staged, journaled once
- [ ] Constraint 3: `collection_not_found` for an unknown or foreign collection, journaled once
- [ ] Constraint 4: no default collection, no top-level upload or copy
- [ ] Constraint 5: upload and copy register the file (home collection, path, object) under the operation ID
- [ ] Constraint 6: last segment is the safe display name, at most 255 bytes
- [ ] Constraint 7: move publishes one snapshot version, no bytes (part 2)
- [ ] Constraint 8: taken path numbered beside it, deleted paths count
- [ ] Constraint 9: `path_suffix_exhausted` after 9,999, no change, journaled failed
- [ ] Constraint 10: `name_too_long` for a numbered segment over 255 bytes
- [ ] Constraint 11: the path used is answered; the journal carries IDs only
- [ ] Constraint 12: `file.upload`, `file.copy`, `file.move` journaled once; refusals once
- [ ] Constraint 13: `navigation.change` published for the changed parents
- [ ] Constraint 14: crash between object and snapshot resumes once under the same operation ID
- [ ] Constraint 15: contract minor bump and regenerated types
- [ ] Constraint 16: API-F2.2 copy remainder closed
- [ ] Native `SqliteCatalog` implements `HomeCollections`; desktop installs it
- [ ] Feature scenarios, walkthrough, plan and checklist boxes
