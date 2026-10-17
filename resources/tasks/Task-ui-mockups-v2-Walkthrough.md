# Task ui-mockups-v2: Walkthrough

Documentation-only task (design artifacts). Output: 37 SVG mockups and an index in [`resources/ui-mockups/v2/`](../ui-mockups/v2/README.md).

## What was done

- Worked out the MVP scope from [C-056](../product-vision.md#release-sequence), [ADR 035](../architecture/035-macos-first-desktop-mvp.md) and phases F0 to F3 of the [implementation plan](../implementation-plan.md), down to the module rows (UI-0.x, UI-F2.1 to UI-F2.11, UI-1.1, UI-1.2, API-F1.3 to API-F2.5, CL-1.1, DE-1.1, DE-1.2, DE-F3.1). Anything assigned to phase F3R, F4 or I1, or with no MVP task, was left out. The v2 README lists what was removed from v1 and why.
- Drew 33 distinct screens plus 4 dark variants at 1440 × 900 in a macOS window, following the [FileSquid Design Guide](../assets/FileSquid-Design-Guide.html) tokens and the v1 layouts.
- Used the coloured logo [`FileSquid-logo.svg`](../assets/FileSquid-logo.svg), at the owner's request after a black-and-white draft: coloured shapes in both modes, with a black outline and wordmark on light surfaces and a white one on dark surfaces.
- Did not mark v2 as the active suite in the [mockups README](../ui-mockups/README.md). The owner approves that, because an active suite becomes an SRS requirement source.

## How the mockups were produced

A Python generator in the session scratch area (not committed, as for v1) holds the tokens, icons and components, so every screen uses the same values. Each SVG was rendered with Chromium and checked for overlap, clipping and token misuse, then fixed and re-rendered.

## Verification

```text
$ for f in resources/ui-mockups/v2/*.svg; do xmllint --noout "$f"; done
(no output: all 37 files parse)

$ ./scripts/verify_isolated_ports.sh       -> 🚀 VALIDATION PASSED
$ ./scripts/verify_doc_sync.sh             -> ✅ Document Sync Validation passed.
$ ./scripts/verify_srs_traceability.sh     -> ✅ Traceability Validation passed: 95 PRD requirements, 81 resolved SRS references.
$ ./scripts/verify_relative_links.sh       -> ✅ Portable Documentation validation passed.
$ ./scripts/verify_workspace_boundaries.sh -> ✅ Workspace Boundary validation passed.
$ ./scripts/verify_centralized_logging.sh  -> ✅ Logging rule validation passed (no relevant staged files).
$ ./scripts/verify_formatting.sh           -> ✅ Format validation passed.
$ ./scripts/verify_docs.sh                 -> ✅ Documentation validation passed.
```

`scripts/verify_branch.sh` (step 0) rejects the session branch name `claude/stoic-gates-famwt6`, which the session was told to push to. The owner decides whether to rename the branch before a pull request.

## Open questions for the owner

- Approve v2 as the active suite, or trim it first.
- Whether the New menu on the desktop should say "Import" (as drawn) or "Upload" (as UI-S-006 words it).
- Whether the People search chip should map to `creator:` on the desktop; v2 leaves it out.
