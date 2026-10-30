# Architecture Decision Record - FileSquid

020-distribution-web-and-tauri.md

## Title

Web container/binary versus Tauri installers; updater out of first scaffold

## Status

Accepted

Accepted: 2026-09-22T07:00:00Z.

Amended 2026-10-17 by [ADR 035](./035-macos-first-desktop-mvp.md): macOS is the first desktop target and the FileSquid MVP is desktop-only; Linux and Windows desktop follow the MVP. The MVP desktop installer is an unsigned, not-notarized macOS `.dmg` labelled early access; the updater stays out of scope until the signing ADR.

Noted 2026-10-29: those Linux and Windows installers follow encrypted backup and pass before the first consumer release ([ADR 029](./029-filesquid-unified-product-and-distributions.md)).

## Context / Requirement Reference

[ADR 002](./002-tauri2-desktop-shell.md) and [ADR 004](./004-axum-hosted-ciphertext-mailbox.md). Public desktop binaries require FOSS/patent review ([ADR 011](./011-native-media-adapters-and-ffmpeg-license.md)). SPA-asset signing is a later gate ([T-034](../product-vision.md#later-gates)).

## Decision

- The owner-operated server ships as an Axum binary and optional container (`apps/filesquid/crates/server/Dockerfile`, crate `filesquid-server`). The ciphertext-only `*.filesquid.tech` mailbox is a separate deployable ([ADR 032](./032-hosting-and-privacy-model.md)). Compose MAY run the server container; it MUST NOT containerize `apps/filesquid/crates/desktop`.
- Desktop ships as Tauri 2 installers after the installer legal review. `bundle.active` is **false** in the scaffold. Amended by [ADR 035](./035-macos-first-desktop-mvp.md): the MVP installer is an unsigned, not-notarized macOS dmg/app labelled early access; NSIS/MSI and AppImage/deb/rpm follow encrypted backup and pass before the first consumer release.
- The updater plugin is **out of scope** until a signing ADR exists.
- Linux and the independent web are CI build-and-test targets from the start. Amended by [ADR 035](./035-macos-first-desktop-mvp.md): this does not make Linux the first desktop target; macOS is.

## Rationale

Avoids claiming a public binary that has not been reviewed.

## Assumptions

- Developers run `npx tauri dev` natively.

## Alternatives Considered

- **Electron / containerized desktop:** Rejected by ADR 002.

## Consequences / Implications

`npx tauri build` is documented separately from `cargo test`. This ADR does not authorize a public download.

## Related Decisions / Notes

- [ADR 002](./002-tauri2-desktop-shell.md), [ADR 011](./011-native-media-adapters-and-ffmpeg-license.md), [ADR 014](./014-stdout-observability.md)
