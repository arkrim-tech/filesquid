# Architecture Decision Record - FileSquid

031-vanilla-typescript-ui-and-white-label-theming.md

## Title

Vanilla TypeScript UIs with standard custom elements, built with Vite, and white-label theming through design tokens, label tables and a distribution manifest

## Status

Accepted

Accepted: 2026-10-15. Supersedes [ADR 003](./003-svelte-client-rendered-ui.md) and the Svelte and per-engine UI package parts of [ADR 028](./028-filesquid-ui-packages-and-panel-composition.md) (Decisions 3 and 7). ADR 028's file-fact demarcation and named panel slots stay in force.

## Context / Requirement Reference

FileSquid has one default UI and several distributions (ImageSquid, later Arkrim, third parties) ([ADR 029](./029-filesquid-unified-product-and-distributions.md)). Every distribution UI MUST be vanilla TypeScript with standard custom elements and no UI framework, styled only through design tokens and worded only through a label table keyed by internal names ([T-051](../product-vision.md#api-and-hosts), [U-29](../product-vision.md#one-api-hosts-and-distributions)). A distribution MUST be definable by tokens (light and dark), labels, brand assets, views and a module set, and missing tokens or labels MUST fail a test ([U-24](../product-vision.md#one-api-hosts-and-distributions), [FE-076](../product-vision.md#platform-api-and-hosts)). FileSquid MUST ship a Drive-style default UI with named sidebar panel slots ([FE-073](../product-vision.md#independent-library), [U-01](../product-vision.md#platform-ownership-and-access)).

[ADR 003](./003-svelte-client-rendered-ui.md) chose Svelte 5 and rejected vanilla HTML, CSS and JavaScript because it "becomes an ad-hoc framework at Photos-class complexity". The owner now requires plain TypeScript UIs. That risk is real and is handled in Decision 6.

The only UI code at writing is the Svelte scaffold in the former `apps/frontend` (logo, host detection, health), which is removed on this branch.

## Decision

**1. Language and build.** UIs are **TypeScript** in strict mode, compiled with **Vite** to static HTML, CSS and JavaScript. Components are standard **custom elements** (`customElements.define`). No UI framework (Svelte, React, Vue, Solid, Lit or similar) is added. The compiled UI MUST NOT need a Node or other server process to render; it is served as static files by `filesquid-server` or bundled into the desktop host.

**2. Packages.**

| Package | Directory | Contents | Dev / preview ports |
|---|---|---|---|
| `@filesquid/ui` | `apps/filesquid/ui/` | `FileSquidClient` API client ([ADR 030](./030-filesquid-api-and-transports.md)), the custom elements (`<fs-app-shell>`, `<fs-file-browser>`, `<fs-metadata-panel>`, `<fs-credentials-panel>`, …), default tokens and labels, and the default Drive-style app | 5173 / 4173 |
| `@imagesquid/ui` | `apps/imagesquid/` | ImageSquid views composed from `@filesquid/ui` elements, ImageSquid tokens, labels, brand and distribution manifest. No API code of its own. | 5174 / 4174 |

A distribution depends on `@filesquid/ui` by path during development and on a published version for release. Views MUST call the API only through `FileSquidClient`.

**3. Design tokens.** Components are styled only through CSS custom properties, with a light and a dark set. Token names follow the existing Arkrim token scheme (`surface`, `on-surface`, `primary`, `outline`, …) with the prefix `--fs-` (for example `--fs-surface`, `--fs-primary`). The default set lives in `apps/filesquid/ui/src/theme/tokens.css`. A distribution supplies its own `tokens.css` that defines every default token. Components MUST NOT hard-code colours, fonts or spacing that a token covers.

**4. Label tables.** Every visible string comes from a JSON label table keyed by internal names (repository, library, collection, virtual collection, virtual exhibit, file, and UI strings). The default table uses the internal names. A distribution supplies its own table; for ImageSquid that is repo, library, album, gallery, exhibit and photo, image or video (ImageSquid terminology). The internal names stay in code, the API and data.

**5. Panels and views.** The named sidebar and preview slots from [ADR 028](./028-filesquid-ui-packages-and-panel-composition.md) are kept. `<fs-file-browser>` exposes them; a view registers the panels it wants (Details, Versions, Metadata, Content Credentials). Panels MUST NOT depend on each other. The file-fact demarcation of ADR 028 (storage facts from `ocfl`, other file facts from `metadata`, credentials from `c2pa`) still decides which panel shows what.

**6. One state and event pattern.** To stop vanilla code from becoming an ad-hoc framework, `@filesquid/ui` MUST document and use one small pattern before ImageSquid views start:

- Data flows down through element properties and attributes.
- Changes flow up as DOM `CustomEvent`s.
- Shared state lives in small store objects that extend `EventTarget` and dispatch a change event; elements subscribe in `connectedCallback` and unsubscribe in `disconnectedCallback`.
- Server state comes only from `FileSquidClient`, including its `subscribe()` events.

Any other state mechanism needs an ADR amendment.

**7. Distribution manifest and desktop branding.** Each distribution has a `distribution.json` (name, bundle identifier, enabled modules, default UI path, Tauri config overlay). The desktop installer for a distribution is built with a Tauri config overlay passed to `tauri build --config <file>`, which sets `productName`, `identifier`, icons and `frontendDist`. `filesquid-server` selects the UI directory from configuration (`FILESQUID_UI_DIR`). One binary per host, many brands.

**8. Tests.** Every element has UI tests that run against built artifacts (`vite build` then `vite preview`, and the packaged desktop app where relevant), never only a dev server. Tests MUST make strict DOM assertions: the element is defined (`customElements.get`), its DOM contains the label from the active table, computed token values match the theme in light and dark, and no console errors occur. A unit test MUST fail when a distribution's label table lacks a key from the default table, or its token file lacks a default token. Runners and reports follow [ADR 017](./017-test-runners-and-reporting.md) (Vitest, Playwright, Allure).

## Rationale

The owner wants UIs that are plain HTML, CSS and TypeScript, so that distribution builders need no framework knowledge and FileSquid carries no framework upgrade cycle. Custom elements are a browser standard: any page, with or without a framework, can embed them, which ADR 028 already wanted through a Web Components build. Vite and strict TypeScript give fast builds and type safety without a runtime.

CSS custom properties and label tables are the smallest mechanism that lets one set of elements carry several brands and vocabularies. Basing token names on the Arkrim scheme reuses a tested colour role set instead of inventing one.

## Assumptions

- Custom elements, Shadow DOM and CSS custom properties work in the three desktop webviews (WebKitGTK, WKWebView, WebView2) and current browsers. UI tests on each confirm it.
- Collection-scale views (virtualised grids, keyboard and screen-reader support) can be built as custom elements with the pattern in Decision 6. Performance is measured under [ADR 007](./007-durable-jobs-and-interactive-performance.md), not assumed.
- Tauri CLI `--config` accepts JSON strings or paths to JSON, JSON5 or TOML files, merged with the default configuration in the order given, later values overwriting earlier ones ([Tauri CLI reference](https://v2.tauri.app/reference/cli/)). The FileSquid overlay path and the pinned CLI version MUST be verified in [DE-F3.1](../../apps/filesquid/crates/desktop/resources/plan.md). The ImageSquid overlay is ImageSquid's own plan (IS-F0.1), not the FileSquid MVP or [DE-F3.3](../../apps/filesquid/crates/desktop/resources/plan.md).
- Vite 8.x and TypeScript 7.x are current on npm at writing (`npm info vite version`: 8.3.4; `npm info typescript version`: 7.0.2). Exact versions are pinned in each package lockfile.

## Alternatives Considered

- **Keep Svelte 5 (ADR 003):** A framework and its upgrade cycle for every distribution builder. Rejected.
- **React, Vue or Solid:** Larger runtimes and the same lock-in. Rejected.
- **Lit or another custom-element library:** Small, but still a framework dependency that distributions must learn. Not adopted; MAY be reconsidered by a later ADR if Decision 6 proves insufficient.
- **Per-engine UI packages (ADR 028):** Three packages and builds for one product. Rejected in favour of one `@filesquid/ui`.
- **Theming through separate CSS files per component or Sass variables:** Cannot be swapped at runtime for light and dark, and does not cross Shadow DOM boundaries. Rejected.

## Consequences / Implications

Easier: no framework to learn or upgrade; elements embed anywhere; a new distribution is tokens, labels, brand and views.

Harder: FileSquid writes its own small rendering and state helpers, which MUST stay within Decision 6; accessibility and virtualisation are built by hand and need rigorous DOM tests; the ImageSquid views are rewritten from the Svelte plan. The Svelte scaffold, its ESLint config and its tests are removed and ported. Lint and docs rules move from Svelte to TypeScript (Prettier, ESLint, TSDoc-style comments) ([ADR 019](./019-coding-style-and-lint.md), [ADR 018](./018-inline-docs-generators.md)).

## Related Decisions / Notes

- Supersedes: [ADR 003](./003-svelte-client-rendered-ui.md). Supersedes in part: [ADR 028](./028-filesquid-ui-packages-and-panel-composition.md) (Svelte, per-engine packages). Amends: [ADR 013](./013-cargo-svelte-workspaces.md) (Vite and TypeScript, no Svelte).
- Related: [ADR 029](./029-filesquid-unified-product-and-distributions.md), [ADR 030](./030-filesquid-api-and-transports.md), [ADR 032](./032-hosting-and-privacy-model.md) (the UI shows which host it is on), [ADR 017](./017-test-runners-and-reporting.md).
- Requirements: [T-051](../product-vision.md#api-and-hosts), [FE-073](../product-vision.md#independent-library), [FE-076](../product-vision.md#platform-api-and-hosts), [U-24, U-29](../product-vision.md#one-api-hosts-and-distributions), [C-055](../product-vision.md#module-rules).
- PRD: [F-055](../prd.md#f-055-default-ui-and-panel-composition), [F-058](../prd.md#f-058-white-label-distributions).
