# Task API-F2.19 ToDos

- [ ] C1 Create, open, copy and rebuild take `{envelope, input}` and answer a `CommandOutcome` for the same operation ID (rename after #139 lands)
- [ ] C2 A stale expected revision and every other 409 refusal is HTTP 409 with a `conflict` outcome beside the `ApiError` fields
- [ ] C3 A repeated operation ID of create, open, copy or rebuild answers its first answer with no second change, before the revision is checked
- [ ] C4 An open of an already-open library, a rebuild that answered an unfinished job and every refusal store no receipt
- [ ] C5 Create expects `0`; open expects `0` or the current revision; copy and rebuild expect the current revision; `LibraryView.revision`
- [ ] C6 The rebuild answers `pending` with its job as `result`; a new job carries the envelope's operation ID; a repeat answers the same job
- [ ] C7 Location-choices and repair keep their bodies; job controls untouched
- [ ] C8 Schema version 1, opaque operation ID, scope `location:<id>` or `library:<id>`; anything else `invalid_request`
- [ ] C9 UI: `ClientLibrarySource` sends through `command()`; `conflict` raised as `ApiError` 409; `rebuild()` resolves to the pending job
- [ ] C10 Contract bumped one minor above main, `schema.d.ts` and `contract-version.ts` regenerated, UI callers moved
- [ ] C11 Every refusal journaled once with its code; no path in any answer or entry
- [ ] C12 Preferences and metadata routes untouched; helper reusable and documented
- [ ] Failing tests first (integration, cucumber feature, UI unit tests)
- [ ] Desktop route tests, steps and e2e spec send envelopes
- [ ] Docs: API srs row API-S-025 (envelope), plan row ticked, checklist ticked, README of the UI, walkthrough; Plan and ToDos deleted
