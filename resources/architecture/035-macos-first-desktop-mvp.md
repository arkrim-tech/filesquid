# Architecture Decision Record - FileSquid

035-macos-first-desktop-mvp.md

## Title

macOS-first desktop target and a desktop-only FileSquid MVP

## Status

Accepted

Accepted: 2026-10-17. Amends [ADR 002](./002-tauri2-desktop-shell.md), [ADR 006](./006-libvips-native-stills-not-imagemagick.md) (its Linux-first profile statement only; libvips workers stay in the MVP), [ADR 013](./013-cargo-svelte-workspaces.md) and [ADR 020](./020-distribution-web-and-tauri.md) where they name Linux desktop as the first target. Does not change the release order of [ADR 029](./029-filesquid-unified-product-and-distributions.md); it inserts an early-access MVP before the first consumer release.

Amended: 2026-10-18 by [ADR 037](./037-c2pa-in-release-2.md), which removes C2PA (read, validation, unsigned import-time records, the Content Credentials routes and panel) from the MVP scope this record cites from C-056.

Amended: 2026-10-19 (owner decision): the app no longer shows an "early access" label; its About view states that the macOS build is not signed or notarised by Apple yet. The unsigned early-access DMG stays the MVP release vehicle and its download page keeps the label (Decision 3).

## Context / Requirement Reference

- Requirements: [C-056](../product-vision.md#release-sequence) (FileSquid MVP scope), [C-025](../product-vision.md#release-sequence) (release sequence), [FE-017](../product-vision.md#cross-platform-access), [FE-075](../product-vision.md#platform-api-and-hosts), [U-34](../product-vision.md#one-api-hosts-and-distributions), [T-011](../product-vision.md#technical-requirements); [PRD F-057](../prd.md#f-057-desktop-and-server-hosts) and the [release order and dependency gates](../prd.md#release-order-and-dependency-gates).
- Earlier ADRs chose Linux desktop and the independent web as the first targets, partly so the weakest webview (WebKitGTK) would be validated first ([ADR 002](./002-tauri2-desktop-shell.md)).
- The owner decided on 2026-10-17 that the first desktop install target is macOS, that Linux and Windows desktop follow the MVP, and that the MVP is desktop-only: no `filesquid-server`, no accounts, groups or sharing.
- The owner also decided that the MVP installer may be an unsigned, not-notarized DMG, so the MVP does not wait on an Apple Developer ID, a signing ADR or the updater.
- Tauri's WebdriverIO service (`@wdio/tauri-service`), already named for native-window tests in [DE-0.1](../../apps/filesquid/crates/desktop/resources/plan.md), runs on Windows, Linux and macOS because it embeds a WebDriver server in the app; driving `tauri-driver` directly does not support macOS (checked against the Tauri 2 WebDriver documentation on 2026-10-17).

## Decision

1. **macOS is the first desktop target.** Installers, packaged-desktop performance evidence and native-window tests start on macOS (WKWebView). Linux (WebKitGTK) and Windows (WebView2) follow the MVP, before the first consumer release.
2. **The FileSquid MVP is desktop-only.** The MVP ships the default distribution on the macOS desktop host with the scope in [C-056](../product-vision.md#release-sequence). `filesquid-server`, its HTTP binding, accounts, groups, sharing, the container image and the multi-user load test move after the MVP. `filesquid-api` stays transport-agnostic ([ADR 030](./030-filesquid-api-and-transports.md)); nothing in the MVP may assume a single transport.
3. **Unsigned early-access DMG.** The MVP installer is a `.dmg` built by the Tauri bundler, not signed with a Developer ID and not notarized. On Apple silicon the bundle MUST still carry an ad-hoc signature so it can launch; the exact Tauri configuration is verified in DE-F3.1 before it is documented ([command-verification](../../.agents/rules/command-verification.md)). The download page labels the build "early access", and the install guide states how to open an app Gatekeeper blocks. The app itself shows no early-access label; its About view states that the build is not signed or notarised by Apple yet (amended 2026-10-19, owner decision). Developer ID signing, notarization and the updater remain gated by a signing ADR (Task 8.5).
4. **Native-window tests on macOS use the WebdriverIO Tauri service.** `tauri-driver` is not used on macOS.
5. **The installer licence review is on the MVP path.** Distributing the early-access DMG counts as a public desktop binary. The [ADR 011](./011-native-media-adapters-and-ffmpeg-license.md) FOSS licence and patent review of the DMG's actual linked inventory (libvips and its loaders included) MUST pass before the DMG is distributed to anyone outside the development team (owner decision, 2026-10-17).

## Rationale

- The owner's first users are on macOS, so validating there first gives the earliest useful feedback.
- A desktop-only MVP removes the server host, multi-user authorisation and sharing, roughly a third of the release-1 work, without changing the API boundary that the server will mount later.
- An unsigned DMG removes an external dependency (Apple Developer Program enrolment and a signing ADR) from the MVP's critical path.

## Assumptions

- MVP users accept an early-access build and can follow the documented steps to open an unsigned app.

## Alternatives Considered

- **Keep Linux first.** Rejected by the owner: the first users are on macOS.
- **Signed and notarized DMG for the MVP.** Rejected for the MVP to avoid waiting on a Developer ID and a signing ADR; it remains required before the first consumer release.
- **Keep a single-user `filesquid-server` in the MVP.** Rejected: it adds a second transport, storage-root configuration and deployment work that MVP users do not need.

## Consequences / Implications

- Easier: one platform, one host and no signing dependency before the MVP.
- Harder: counsel review of the macOS DMG's linked inventory is on the MVP critical path, so it should start as soon as the MVP's native inventory is known (after Task 0.5).
- Harder: WebKitGTK and WebView2 differences surface later; the Linux and Windows installers and their native-window suites become post-MVP work that MUST pass before the first consumer release. Developers on Linux or Windows can build and test the workspace but cannot produce or smoke-test the MVP DMG, which needs a macOS machine.
- Gatekeeper warnings on the unsigned DMG make the MVP unsuitable for a general public launch; the PRD gate says so.
- libvips workers ([ADR 006](./006-libvips-native-stills-not-imagemagick.md)) stay in the MVP, so the MVP DMG bundles native libraries.
- The implementation plan splits Phase F3 into the MVP gate (Task FS-3.9) and a post-MVP completion phase that restores the deferred scope before the first consumer release.

## Related Decisions / Notes

- [ADR 002](./002-tauri2-desktop-shell.md) (Tauri 2 desktop host), [ADR 006](./006-libvips-native-stills-not-imagemagick.md) (libvips), [ADR 011](./011-native-media-adapters-and-ffmpeg-license.md) (installer review), [ADR 013](./013-cargo-svelte-workspaces.md) (workspaces and first targets), [ADR 020](./020-distribution-web-and-tauri.md) (distribution), [ADR 029](./029-filesquid-unified-product-and-distributions.md) (release order), [ADR 030](./030-filesquid-api-and-transports.md) (API and transports), [ADR 032](./032-hosting-and-privacy-model.md) (host privacy model).
- Open: the signing ADR (Developer ID, notarization, updater keys) is still required before the first consumer release.
