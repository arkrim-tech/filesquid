# Task API-F1.4 ToDos

- [ ] Gate 1: one codec. `commit_file_original` and `read_file_original` use `FileRecord`; the old four-line form is gone; canonical tests updated
- [ ] Gate 2: an upload creates `files/<file-id>` with the original and its record in v1; a size mismatch commits nothing; a re-upload is a new version with one more import entry
- [ ] Gate 3: content and download answers use the record's display name and media type
- [ ] Gate 4: statements are read, set and removed; unknown `dcterms:` and unregistered prefixes are refused; one version per edit
- [ ] Gate 5: namespace registration and listing in the snapshot object; the term add order
- [ ] Gate 6: the read-only facts projection, without absolute paths
- [ ] Contract, schema.d.ts, feature, SRS (API-S-026 is already amended), README, walkthrough
- [ ] fmt, clippy, tests, rustdoc, FULL hook
