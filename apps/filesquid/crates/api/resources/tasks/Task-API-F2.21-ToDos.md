# Task API-F2.21 ToDos

- [x] 9. Sub-code on `ApiError` for the four rule codes (and `name_too_long`, `invalid_path`), on upload, copy, move, folder, import and restore refusals; contract schema `ErrorSubCode`
- [x] 8. `history_bytes` on the library history size, each content once, zero with one version per object, same bounds and `truncated`
- [ ] 6. `collection_id` on collection and folder navigation nodes, `null` on other kinds
- [ ] 7. Destination route: collection and folder answers, `invalid_request` for other kinds, one `not_found` for unknown, malformed, unreadable, under-unreadable and removed nodes; journaled once without names
- [ ] 1. File rows of the view carry `library_id`, head version, `collection_id` and collection title (both `null` without a collection)
- [ ] 1. File rows of the search carry the same facts (API-S-012)
- [ ] 2. Folder rows carry files and bytes, within 10,000 files, marked incomplete on a bound or an unreadable file
- [ ] 3. Collection rows carry their `collection_id`
- [ ] 4. `view=recent`: own succeeded entries only, newest first, action kind and RFC 3339 UTC time, 20 journal pages per library; other views still refused
- [ ] 5. View bounds and cursor rules unchanged for the new rows
- [ ] 10. Redaction tests: no name, title or path in the journal
- [ ] 11. Contract minor bump, regenerated `schema.d.ts`, UI typecheck and tests
- [ ] Cucumber steps for the feature file; SRS, plan, checklist, README and walkthrough
