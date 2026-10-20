# Task DE-0.5 Walkthrough: Bounded media workers (libvips stills)

Branch: `feat/de-0.5-media-workers`, built on [Task DE-0.8](./Task-DE-0.8-Walkthrough.md) (global [Task 0.5](../../../../../../resources/implementation-plan.md#task-0.5)).

## Requirements

- **NFR-002 Benchmark envelope and responsiveness** and **NFR-003 Bounded processing and migration endurance**: heavy decode work runs outside the interactive path, CPU-heavy workers ≤ 2, and every queue is finite.
- **F-003 Browsing, selection and viewing**: thumbnails and display-sized derivatives.
- **ADR 006 libvips for native stills, not ImageMagick**: libvips runs in out-of-process workers, and ImageMagick is never a dependency, sidecar or fallback, Magick loaders included.
- **DE-S-016 Bounded media workers** (new): the rules below.

## What changed

- `filesquid-native` (desktop feature) gains:
  - **`workers::media::MediaWorkers`**, the host-side pool:
    - at most `DEFAULT_CPU_HEAVY_WORKERS` (2) workers at once;
    - at most `MAX_QUEUED` (64) waiting callers, each waiting no longer than the run time limit, after which the answer is `busy`;
    - each run killed past its limit (`DEFAULT_WORKER_TIMEOUT`, 60 s);
    - each run registered as a `media.thumbnail` process, completed or failed with an error code (`worker_crashed`, `worker_timeout`, `unsupported_format`…), which the registry journals;
    - answers and entries never carry a path;
    - requests are checked before any process starts: an edge of 1 to 4,096 px, and output as PNG, JPEG or WebP.
  - **`worker_main`**: one worker run, one OS process. It reads one JSON request on stdin and writes one JSON answer line on stdout. It runs inside the desktop executable: `filesquid-desktop --filesquid-media-worker` (`WORKER_ARG`) runs the worker and exits before Tauri, any window or the catalogs start. A first version shipped the worker as its own binary target. The `filesquid-cli` gate (F-050: `fsquid` is the only CLI binary) rejected it, so the worker moved into the host executable, which also leaves the installer only one program to ship.
  - **`workers::media::vips`**, the worker side:
    - It loads libvips 8 at run time through `libloading`, already in the lockfile at 0.7.4, now an optional dependency enabled only by `desktop`. No build needs libvips headers, and a machine without libvips answers `libvips_unavailable`.
    - It calls `vips_init`, then blocks `VipsForeignLoadMagick` and `VipsForeignSaveMagick` with `vips_operation_block_set` before it reads a file. A libvips without that call (older than 8.13) is refused.
    - It runs `vips_thumbnail` and `vips_image_write_to_file`.
    - Every foreign call is in one type and names the C signature it relies on.
- `filesquid-desktop`: `main` dispatches `--filesquid-media-worker` to the worker. `media_workers(host)` builds the pool over the host's own executable with that argument, and launch keeps it as `DesktopMedia` in Tauri's managed state. No worker starts at launch.
- The development machine's libvips 8.15 carries the ImageMagick module (`vips-magick.so`). The block is proven to matter: with it removed, the BMP test decodes the file through ImageMagick and fails, as recorded below.
- README: libvips is now a documented runtime prerequisite, and ImageMagick must not be installed.

## SRS traceability

| Requirement | Tests |
|---|---|
| DE-S-016 Bounded media workers; F-003 Browsing, selection and viewing: thumbnail within the bound, by a separate process | desktop `integration_media_workers::the_desktop_executable_makes_a_thumbnail_out_of_process` (the built executable with the worker argument: 64×48 PPM → 16×12 PNG; ImageMagick reported blocked); native `unit_media::the_worker_writes_a_thumbnail_within_the_requested_bound`; feature "The libvips worker makes a thumbnail within the requested bound" |
| ADR 006 libvips for native stills, not ImageMagick: no ImageMagick fallback | native `unit_media::the_worker_blocks_the_imagemagick_loaders_and_refuses_a_magick_only_format`, `no_imagemagick_crate_is_in_the_dependency_graph`; feature "ImageMagick is never a fallback" |
| NFR-003 Bounded processing and migration endurance; DE-S-016: at most two workers | native `unit_media::at_most_two_workers_run_at_once` (six requests; peak 2; three waves); desktop `integration_media_workers::the_host_runs_at_most_two_media_workers_from_its_own_executable`; feature "At most two workers run at once" |
| DE-S-016 Bounded media workers: crash isolation, registry and redaction | native `unit_media::a_crashed_worker_fails_its_run_only_and_is_journaled_without_paths`; feature "A crashed worker fails only its own run" |
| DE-S-016 Bounded media workers: time limit | native `unit_media::a_worker_past_its_time_limit_is_stopped` |
| DE-S-016 Bounded media workers: request checks | native `unit_media::a_missing_source_an_unknown_output_format_or_a_bad_request_is_refused` (refused before any process starts) |
| DE-S-016 Bounded media workers: worker invocation | native `unit_media::only_the_worker_argument_starts_a_worker_run`; the F-050 gate `filesquid-cli` `features` ("fsquid is the only CLI binary") passes |

## TDD: failing runs

```text
error[E0432]: unresolved import `filesquid_native::workers::media`
error: could not compile `filesquid-native` (test "unit_media") due to 1 previous error
```

With the ImageMagick block removed from the worker, to show the test catches a Magick fallback:

```text
test the_worker_blocks_the_imagemagick_loaders_and_refuses_a_magick_only_format ... FAILED
  left: None
 right: Some(Refused("unsupported_format"))
```

## Known gaps

- **Packaging**: the installer must ship a libvips built without its Magick module beside the app; the worker itself is inside the desktop executable. This belongs with the installer work (DE-F3.x). Until then, a machine without libvips answers `libvips_unavailable`.
- **Routes**: the thumbnail and preview routes that call `DesktopMedia` come with DE-1.3. Caching of derivatives (versioned keys and finite budgets, F-003) belongs there too.
- **Memory**: each worker is its own process, so its memory is returned when it exits. A per-process memory cap (`ulimit` or a job object) is not set yet.

## Mocked Environment Variables

None.

## Raw test output

```text
$ cargo test -p filesquid-native --features desktop --test unit_media --locked
running 8 tests
test a_missing_source_an_unknown_output_format_or_a_bad_request_is_refused ... ok
test no_imagemagick_crate_is_in_the_dependency_graph ... ok
test only_the_worker_argument_starts_a_worker_run ... ok
test a_crashed_worker_fails_its_run_only_and_is_journaled_without_paths ... ok
test the_worker_writes_a_thumbnail_within_the_requested_bound ... ok
test the_worker_blocks_the_imagemagick_loaders_and_refuses_a_magick_only_format ... ok
test a_worker_past_its_time_limit_is_stopped ... ok
test at_most_two_workers_run_at_once ... ok
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.94s
$ cargo test -p filesquid-desktop --test integration_media_workers --locked
running 2 tests
test the_host_runs_at_most_two_media_workers_from_its_own_executable ... ok
test the_desktop_executable_makes_a_thumbnail_out_of_process ... ok
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.16s
$ cargo test -p filesquid-native --features desktop --test catalog_features --locked
Feature: Intent, OCFL snapshot commit and receipt reconcile
  Scenario: A change is published as an OCFL version before it is acknowledged
  Scenario: A crash before the snapshot leaves the work pending
  Scenario: A crash after the snapshot loses no acknowledged state
  Scenario: A lost catalog is rebuilt from the OCFL snapshot
Feature: Bounded, out-of-process media workers
  Scenario: The libvips worker makes a thumbnail within the requested bound
  Scenario: ImageMagick is never a fallback
  Scenario: At most two workers run at once
  Scenario: A crashed worker fails only its own run
Feature: Per-library SQLite catalog
  Scenario: Each library keeps its own catalog file
  Scenario: Organisation survives a restart
  Scenario: A retried command is answered once
  Scenario: A command that changes nothing stores nothing
  Scenario: An index refresh that changes nothing journals nothing
  Scenario: Journal entries survive an unreachable host journal
  Scenario: A command that cannot be stored changes nothing
  Scenario: A catalog is an index that a backup copies consistently
16 scenarios (16 passed)
83 steps (83 passed)
$ cargo test -p filesquid-cli --test features --locked
7 scenarios (7 passed)
16 steps (16 passed)
```

The head-commit hook run is recorded in the pull request.
