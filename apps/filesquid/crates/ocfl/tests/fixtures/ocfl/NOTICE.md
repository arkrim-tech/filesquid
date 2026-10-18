# OCFL community fixtures (pinned test data)

This directory holds an unchanged copy of the `1.1/` tree of the OCFL community test fixtures, used only by the `filesquid-ocfl` tests ([OCFL-S-009 to OCFL-S-011](../../../resources/srs.md)).

| Field | Value |
|---|---|
| Source | https://github.com/OCFL/fixtures |
| Pinned commit | `d323fc207126becec066998b43408d998bf0fb1b` |
| Commit date | 2024-12-05T17:25:06Z |
| Copied | `1.1/` (good-objects, warn-objects, bad-objects, content), `README.md`, `.zenodo.json` |
| Not copied | `1.0/` (OCFL 1.0 objects are out of scope, OCFL-S-011) |
| Licence | Creative Commons Attribution 4.0 International (CC-BY-4.0), as stated in the upstream `.zenodo.json` |

## Attribution

"Oxford Common File Layout - Fixtures" by the OCFL community and editors (https://ocfl.io), licensed under CC-BY-4.0 (https://creativecommons.org/licenses/by/4.0/). The files under `1.1/`, `README.md` and `.zenodo.json` are copied without changes. This `NOTICE.md` is the only file added.

## Rules

- The files are test data. They are excluded from the crate package (`exclude` in `Cargo.toml`), and no library or CLI code loads them.
- Do not edit them. Moving the pin replaces the whole tree from another upstream commit in its own reviewed change, and reruns the validator, read and write conformance tests (OCFL-S-011).
- The tests read this tree by default. `FILESQUID_OCFL_FIXTURES` may name another copy of a `1.1/` tree, for example to check a new pin before it is committed.
