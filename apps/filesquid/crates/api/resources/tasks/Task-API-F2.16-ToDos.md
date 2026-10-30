# Task API-F2.16 ToDos

| # | Gate | Constraint |
|---|---|---|
| 1 | `unit_imports_route`: planned answer with every plan field before any byte is staged | 1 |
| 2 | `unit_imports_route`: plan items paged, base names only, 1,000 cap | 2 |
| 3 | `unit_imports_route`: start once, repeated start same job | 3 |
| 4 | `unit_imports_route`: DELETE and expiry discard with no change | 4 |
| 5 | `unit_imports_route`: `source_changed` refused | 5 |
| 6 | `unit_imports_route`: replay of open, started, cancelled, expired | 6, 7 |
| 7 | `unit_imports_route`: journal `import.plan`, `import.start`, `import.discard` once, no name | 8 |
| 8 | `unit_contract`, `npm run contract:types`: 0.31.0 | 9 |
| 9 | `unit_job_items`: view carries destination | 10 |
| 10 | `unit_job_items`: items paged with fields, never a path | 11 |
| 11 | `unit_job_items`: retry one and retry-failed, paused refused, repeated no-op, unknown item 404 | 12 |
| 12 | `unit_job_items`: `job.item_retried` once per file, no name | 13 |
| 13 | `features`: BDD feature for API-S-036 and API-S-037 | all |
| 14 | Desktop still compiles and its import tests pass | design |
| 15 | Walkthrough, plan row, checklist, progress block | workflow |
