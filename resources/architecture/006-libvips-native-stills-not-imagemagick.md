# Architecture Decision Record - FileSquid

006-libvips-native-stills-not-imagemagick.md

## Title

libvips as the only stills processing engine; do not install ImageMagick

## Status

Accepted

Accepted: 2026-09-16T01:01:07Z. Revised: 2026-09-22T04:34:37Z. Revised: 2026-10-15T05:24:52Z. Amended by [ADR 026](./026-filesquid-provenance-engine.md): C2PA is the from-scratch `filesquid-c2pa` client engine, not the official `c2pa` crate.

Amended 2026-10-17 by [ADR 035](./035-macos-first-desktop-mvp.md): macOS is the first desktop target and the FileSquid MVP is desktop-only; Linux and Windows desktop follow the MVP.

## Context / Requirement Reference

Desktop workers MUST generate thumbnails and display-sized previews with bounded memory while the UI stays responsive ([U-11](../product-vision.md#platform-ownership-and-access), [T-010](../product-vision.md#performance-acceptance), [T-015](../product-vision.md#interactive-rendering), [U-26](../product-vision.md#one-api-hosts-and-distributions)). Originals stay byte-exact; conversions are linked renditions ([C-028](../product-vision.md#platforms-originals-and-service-limits), [FE-001](../product-vision.md#independent-library)). The owner asked whether libvips fully replaces ImageMagick and stated that only one of the two may be installed.

Native tools MUST ship inside per-OS FileSquid installers so users do not install a separate ImageMagick or FFmpeg stack.

## Decision

**Install libvips. Do not install ImageMagick.** ImageMagick MUST NOT be a runtime dependency, sidecar, or fallback stills engine.

libvips is the stills engine, linked into **out-of-process** native workers (separate OS processes, not only threads) so a decoder crash cannot take down the catalog or UI ([U-26](../product-vision.md#one-api-hosts-and-distributions), [FE-064](../product-vision.md#desktop-host-isolation)). libvips cache limits are not total process-memory limits ([libvips cache](https://www.libvips.org/API/current/func.cache_set_max_mem.html)). Workers ship in the Tauri installer for each OS/CPU.

Packaged builds and the dependency inventory MUST **exclude ImageMagick**, including optional libvips Magick loaders and transitive dependencies ([libvips build](https://github.com/libvips/libvips/blob/master/README.md)). CI MUST fail if ImageMagick/MagickWand appears.

libvips is used for:

- decode of declared still formats for display
- thumbnail and display-sized derivative generation
- streaming resize, crop, rotate, orientation and ICC-aware colour conversion
- bounded-memory handling of large stills

libvips is **not** a 100% clone of ImageMagick's feature surface (hundreds of niche formats, FX scripting, drawing, animated-GIF toolbox). It **is** the replacement for the role ImageMagick would have played in FileSquid. Gaps are filled by dedicated libraries, not by keeping ImageMagick "just in case":

| Need | Library | Not solved by libvips-or-ImageMagick alone |
|---|---|---|
| Video/audio | FFmpeg in an isolated worker (license review later) | Yes |
| RAW development | LibRaw or successor adapter (later ADR) | Yes |
| HEIF/HEIC | libheif or OS decoder (later ADR, per-OS tests) | Loader-dependent |
| C2PA read/write | `filesquid-c2pa` client engine ([ADR 026](./026-filesquid-provenance-engine.md)) | Yes |
| EXIF/IPTC/XMP product | Dedicated metadata adapters ([FE-054](../product-vision.md#metadata-ownership-and-sharing-privacy)) | Yes |
| Non-destructive editor | FileSquid render graph | Yes |
| Obscure still formats | Preserve bytes; show unsupported | Do not add ImageMagick for soup |

If a **required** stills format cannot be decoded by libvips plus its declared loaders, a later ADR MAY add a **dedicated** decoder. It MUST NOT add ImageMagick as a second kitchen-sink.

FFmpeg, LibRaw and libheif are compiled or bundled **into** the same Rust workers / installer; `filesquid-c2pa` is linked as a client engine ([ADR 026](./026-filesquid-provenance-engine.md)). Users do not run those tools as separate backend services. PhotoKit and other OS APIs remain platform adapters, not portable substitutes.

## Rationale

FileSquid needs fast, streaming, low-memory derivatives for large libraries. That is libvips' purpose. ImageMagick's extra format/FX surface is not a product requirement, and its parser surface has a long security history. Running both would violate "only one" and would double native-build cost.

Shipping libraries inside per-OS installers is the cross-platform path: one shared Rust source, CI builds for Linux, macOS and Windows. It is not "write once, one binary forever," and it is not "call whatever the user installed on PATH."

## Assumptions

- libvips plus declared loaders cover JPEG, PNG, TIFF and WebP on the **desktop** profiles, macOS first (the MVP DMG, [ADR 035](./035-macos-first-desktop-mvp.md)) and then Linux and Windows, with HEIF/RAW behind explicit adapters. This coverage is unverified on every OS until the libvips worker tests ([DE-0.5](../../apps/filesquid/crates/desktop/resources/plan.md)) pass against the packaged loaders of that OS; the enabled loaders are recorded in a later implementation ADR. Desktop libvips TIFF (or any native decode) does **not** establish browser decoding; ADR 008 excludes that native pipeline from WASM. Browser viewing uses prepared, browser-decodable derivatives or a visible unsupported state.
- Static or bundled linking of native deps is achievable in CI for each supported target; FFmpeg license/codec flags get their own ADR before video processing ships.
- Unsupported formats remain preserved originals with a visible capability state.

## Alternatives Considered

- **ImageMagick only:** Rejected for memory, security surface, and weaker streaming for large stills.
- **Both ImageMagick and libvips:** Rejected by the owner and by the no-redundancy rule.
- **Pure Rust image crates only:** Insufficient today for the format/colour/performance envelope; may assist small cases but do not replace libvips.
- **Shell out to user-installed CLIs:** Rejected; versions and presence differ per OS.
- **Server-side ImageMagick on plaintext:** Forbidden by the mailbox model ([ADR 004](./004-axum-hosted-ciphertext-mailbox.md)).

## Consequences / Implications

Easier: one stills engine; one installer story. Harder: native build matrices; process isolation and worker budgets; FFmpeg and other bundled licenses. Animation-as-stills (GIF) is not a reason to bring ImageMagick back; treat motion via FFmpeg or a declared limitation.

Supported stills decode MUST have fidelity fixtures for orientation, colour profiles, transparency, bit depth, multi-page images and malformed inputs. The **entire** bundled native inventory (not only FFmpeg) requires license/distribution review before a public desktop binary. That FOSS/patent checklist lives in [ADR 011](./011-native-media-adapters-and-ffmpeg-license.md) (libvips is in the same installer).

This ADR sets direction. Exact crate/sys bindings, enabled libvips loaders, FFmpeg build flags and LibRaw versions are later implementation ADRs in the **same language**.

## Related Decisions / Notes

- [ADR 001](./001-shared-rust-core-with-tauri-and-axum-hosts.md), [ADR 002](./002-tauri2-desktop-shell.md), [ADR 007](./007-durable-jobs-and-interactive-performance.md), [ADR 008](./008-browser-wasm-scope.md), [ADR 011](./011-native-media-adapters-and-ffmpeg-license.md)
- [libvips](https://www.libvips.org/), [c2pa crate](https://crates.io/crates/c2pa), [libvips Magick optional](https://github.com/libvips/libvips/blob/master/README.md)
