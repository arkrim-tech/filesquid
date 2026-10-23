# Task API-F2.18 ToDos

- [x] Branch `feat/api-f2.18-library-facts` from a fresh `origin/main`
- [x] Plan and ToDos with every SRS constraint
- [ ] Failing tests: create with a name of 1 to 1,024 bytes; blank, too long or control characters refused as `invalid_name`
- [ ] Failing tests: rename journaled `library.rename` with the library ID only; same name not journaled; unknown library 404
- [ ] Failing tests: facts answer `library_id`, `name`, `free_bytes`, `free_bytes_reason`; null with `volume_unreadable` or `not_local`
- [ ] Failing tests: facts and recent are 404 / filtered for a library the caller may not read; a non-owner who may read sees free bytes
- [ ] Failing tests: recent keeps readable libraries first, then at most 20 newest first, with last opened (RFC 3339 UTC) and `open`; never a path or location ID
- [ ] Redaction tests: no name, path or location ID in a journal entry or error
- [ ] Port `LibraryRegistry`, `RegistryBook` (bounded 256), `MemoryLibraryRegistry`
- [ ] Desktop: `HostLocations` implements the port; free bytes by `fs4::available_space` on Linux and macOS, `volume_unreadable` elsewhere
- [ ] Server: the routes answer 503 `unavailable`
- [ ] Contract 0.26.0, `npm run contract:types` (schema and contract version)
- [ ] BDD feature for API-S-038
- [ ] Docs: plan rows (API-F2.18 and DE-1.8 Status), checklist, progress, walkthrough; delete Plan and ToDos
- [ ] `FULL=1 bash scripts/pre-commit.sh` on the head commit
