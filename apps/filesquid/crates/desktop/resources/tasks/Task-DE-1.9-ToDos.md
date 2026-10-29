# Task DE-1.9 ToDos

- [ ] Failing tests first (native unit, desktop integration), run and recorded
- [ ] `import` classified as a transfer (cap 4)
- [ ] `ImportWorker` as the `import` job runner, installed before the router is served
- [ ] Control read after every per-file checkpoint and lease renewal; halt only via `halt_if_requested`
- [ ] Pause: pending, paused, checkpoint after the file in hand
- [ ] Cancel: cancelled after the file in hand, no further read, plan finished, intent cleared
- [ ] Resume before the checkpoint keeps the run going
- [ ] Replay skips a paused job without a refusal entry
- [ ] Controls journaled once (by the API only)
- [ ] Ordered `job.update` sequence, no event dropped
- [ ] No source path or name in answers, events, errors or journal
- [ ] API import commit streams through `stage_file`; source read at most twice
- [ ] `source_changed` retryable under the attempt rule; `too_large_for_this_build` removed
- [ ] File above 512 MiB imported within bounded memory
- [ ] BDD scenarios
- [ ] Docs: DE-S-019 sentence, crate README/root README if needed, plan and checklist ticks, progress block
- [ ] fmt, clippy, tests, doc; pre-commit; walkthrough; delete Plan and ToDos
