# @filesquid/ui

FileSquid's default distribution and the component library every distribution builds on: the `FileSquidClient` API client and host adapter, vanilla TypeScript custom elements (`<fs-app-shell>`, `<fs-monitor>`, `<fs-nav-tree>`, `<fs-breadcrumb>`, `<fs-states>`, `<fs-host-indicator>`), design tokens and label tables ([ADR 031](../../../resources/architecture/031-vanilla-typescript-ui-and-white-label-theming.md)). Requirements: [SRS](./resources/srs.md). Plan: [plan](./resources/plan.md).

## Develop and test

From `apps/filesquid/ui`, on any OS with Node.js 22.12 or later:

```sh
npm ci
npm run dev            # http://127.0.0.1:5173 (preview: 4173)
npm run build          # type-check, then the static build in dist/
npm test               # Vitest unit and jsdom UI tests; Allure and JUnit in tests/allure-results/
npm run test:e2e       # Playwright: built UI on a real filesquid-server, then built UI on vite preview
npm run report         # Allure HTML report from tests/allure-results/ into tests/allure-report/
npm run contract:types # regenerate src/api/schema.d.ts from crates/api/openapi.json
```

`npm run test:e2e` builds the UI and starts `filesquid-server` with Cargo, so it needs the Rust toolchain and a Playwright browser (`npx playwright install chromium`). The repository's pre-commit hook runs it when `FILESQUID_E2E=1` is set. The second suite, `tests/e2e/preview/`, serves the static build with `vite preview` on port 4173 and needs no server; both use `tests/e2e/support/strict-dom.ts` for strict DOM assertions (defined elements, labels from the label table, computed tokens in light and dark, no console errors).

## Theme, tokens and brand

Components are styled only through `--fs-` custom properties ([SRS-UI-007](../../../resources/srs.md#srs-ui-007-light-and-dark-themes), [UI-S-015](./resources/srs.md)). [`src/theme/tokens.css`](./src/theme/tokens.css) holds FileSquid's values from the [FileSquid Design Guide](../../../resources/assets/FileSquid-design-guide-v1.html), adjusted for WCAG 2.2 AA: white text on the brand green `#16ac49` is only 2.99:1, so the light primary is `#0b7a32` and the dark theme keeps the brand green with dark ink (`#0d2b18`). A distribution imports these defaults first and its own `tokens.css` on top. It defines every token in `:root` (light). It defines every colour token again in both `:root[data-theme="dark"]` and the `prefers-color-scheme: dark` block; the theme-independent tokens (fonts, type sizes, spacing, radius and motion, listed in `THEME_INDEPENDENT_TOKENS`) need not be repeated there. Colour tokens are `#rgb` or `#rrggbb`. Component stylesheets take colour, type size, spacing and radius only from tokens, and a unit test fails on a literal value.

| Token                                                                 | Role                                    |
| --------------------------------------------------------------------- | --------------------------------------- |
| `--fs-surface`, `--fs-surface-container`                              | Page and sidebar or panel backgrounds   |
| `--fs-on-surface`, `--fs-on-surface-variant`                          | Body text and secondary text            |
| `--fs-outline`                                                        | Dividers and borders                    |
| `--fs-primary`, `--fs-on-primary`                                     | The primary action and the text on it   |
| `--fs-primary-container`, `--fs-on-primary-container`                 | A selected item and its text            |
| `--fs-error`                                                          | Error text                              |
| `--fs-font-sans`, `--fs-font-heading`, `--fs-font-mono`               | Interface, heading and identifier fonts |
| `--fs-font-size-small`, `--fs-font-size-body`, `--fs-font-size-title` | Type sizes                              |
| `--fs-space-1` to `--fs-space-5`                                      | Spacing (FileSquid: 4, 8, 12, 16, 24px) |
| `--fs-radius`                                                         | Corner radius                           |
| `--fs-motion-duration`                                                | Transitions and animations (150ms)      |

A distribution's own tests check its sheet with the helpers from `@filesquid/ui/theme`:

```ts
import {
  contrastFailures,
  invalidColourTokens,
  missingLabels,
  missingTokens,
} from "@filesquid/ui/theme";

expect(missingTokens(tokensCss)).toEqual({ light: [], dark: [] });
expect(invalidColourTokens(tokensCss)).toEqual([]); // every colour token is #rgb or #rrggbb
expect(contrastFailures(tokensCss)).toEqual([]); // text pairs 4.5:1, focus-ring pairs 3:1, both themes
expect(missingLabels(labels)).toEqual([]);
```

**Theme choice.** `createThemeStore(window)` returns the shared `ThemeStore`; call `start()` before configuring the shell. With no saved choice the theme follows the operating system and the root element has no `data-theme`. `<fs-theme-control>` in the shell header offers System, Light and Dark as native radio buttons (arrow keys move between them); Light or Dark sets `data-theme` and is kept on the device in Web Storage under `filesquid.theme`, so it survives a relaunch. System removes both. The choice is a display preference, never a credential; if the browser refuses storage, the choice applies until the page closes and the console says why.

**Brand slots.** The distribution passes its product name and logo to `<fs-app-shell>`. `logo.srcDark` is an optional variant for dark surfaces: FileSquid uses the official mark with a black outline ([`public/filesquid-mark.svg`](./public/filesquid-mark.svg)) and a white one in the dark theme ([`public/filesquid-mark-dark.svg`](./public/filesquid-mark-dark.svg)), both cropped from the official [`FileSquid-logo.svg`](../../../resources/assets/FileSquid-logo.svg) without the wordmark.

```ts
const theme = createThemeStore(window);
theme.start();
defineElements();
shell.configure({ distribution, client: createHostClient(window), theme });
```

## Accessibility contracts

Every custom element meets the contracts of [UI-S-016](./resources/srs.md) ([SRS-NFR-006](../../../resources/srs.md#srs-nfr-006-accessibility-and-usability)), so a distribution inherits them by composing the elements and supplying its tokens and labels:

- **Keyboard.** Everything is a native control (`a[href]`, `button`, `input`); nothing has a positive `tabindex`. The shell's Tab order is the skip control, the theme control (one stop; arrow keys choose), the navigation (links, or a slotted tree's controls), then the main area.
- **Skip control and landmarks.** The first Tab shows a "Skip to main content" button (`skipToContent` label) that focuses `main` without touching the hash route. `header`, a `nav` named by the `navigationMain` label and `main` are the landmarks; the product name is the only `h1`. Monitor tables are named by their section heading inside focusable scroll regions.
- **Focus.** Every element in the Tab order shows a `2px` solid `--fs-primary` outline offset by `2px`; `contrastFailures` keeps `--fs-primary` at 3:1 on `--fs-surface`, `--fs-surface-container` and `--fs-primary-container` (`FOCUS_CONTRAST_PAIRS`). A route change moves focus to `main`; a Monitor re-render moves it to the token field, the alert or the first section heading. Nothing takes focus on first load, and a background load, poll or failed poll never takes focus from outside the content it replaces.
- **Motion and contrast preferences.** Durations come from `--fs-motion-duration` and are `0s` under `prefers-reduced-motion: reduce`; script-driven motion checks `prefersReducedMotion(window)`. Under `prefers-contrast: more` borders use `--fs-on-surface-variant`; under `forced-colors: active` the chosen theme option uses `Highlight` and focus rings use `Highlight`.
- **Text size and reflow.** Type sizes are `rem` tokens and no text container has a fixed pixel height. At 320 CSS px, at 200 % text, and at both together, the page never scrolls sideways; below `40rem` the navigation stacks above `main`.
- **Locale.** A distribution may set `locale: { lang, dir }` (default `en`, `ltr`); `resolveLocale` trims the language, writes it in canonical BCP 47 form (`en-us` becomes `en-US`) and throws at startup, naming the distribution, for a malformed tag or a direction other than `ltr` or `rtl`; `applyLocale(document, resolveLocale(distribution))` writes them to `<html>`. Component stylesheets use logical properties only, so `dir="rtl"` mirrors the layout.

The shared rules live in [`src/components/a11y-style.ts`](./src/components/a11y-style.ts) (`A11Y_STYLE`), which every component puts first in its shadow root. Shadow-root styles cannot reach light-DOM content a distribution slots into a component, so `<fs-app-shell>` also adds one document-level copy (`<style id="fs-a11y-style">` in the head) while it is connected, and the last shell to disconnect removes it. The helpers that manage it are internal. A distribution's own UI tests can audit what it renders:

```ts
import { accessibilityViolations } from "@filesquid/ui";

expect(accessibilityViolations(shell)).toEqual([]); // positive tabindex, unnamed controls, missing alt, unnamed landmarks
```

`npm test` covers the helpers and component states in jsdom; `npm run test:e2e` checks Chromium's accessibility tree, Tab order, focus rings in light and dark, slotted content, reduced motion, contrast preferences, reflow (320 px at 100 % and 200 % text, 640 px and 1280 px at 200 %) and RTL against the built UI ([`tests/e2e/accessibility.spec.ts`](./tests/e2e/accessibility.spec.ts)).

## Navigation, breadcrumb and view states

These elements need no API route of their own (Task UI-F2.1a in the [plan](./resources/plan.md), described in [Task-UI-F2.1a-Walkthrough.md](./resources/tasks/Task-UI-F2.1a-Walkthrough.md)). Data flows down as properties and changes flow up as events ([ADR 031](../../../resources/architecture/031-vanilla-typescript-ui-and-white-label-theming.md) Decision 6); none of them fetches by itself.

- **`<fs-states>`** shows one `ViewState` instead of content: `loading` and `empty` in a polite `role="status"` region, `error`, `offline` and `denied` (with the denial reason's label) in a `role="alert"` region. Both regions exist from the first render and are updated in place. Error and offline offer Retry, which dispatches a bubbling, composed `fs-retry` event. `failureState(error, navigator.onLine)` picks offline, denied or error for a failed load.
- **`<fs-host-indicator>`** (Task UI-F2.1b, part 1; [UI-S-004 and UI-S-018](./resources/srs.md)) is the persistent host privacy indicator, mounted by `<fs-app-shell>` in its header so it shows on every view; the shell's client must therefore implement `HostClient` (`host()` and `subscribe(listener, onOpen?)`, which `FileSquidClient` does). It takes `{ labels, client }` and takes its facts only from the host descriptor (`GET /api/v1/host`, read with `client.host()`): at start, replaced by the whole descriptor of each `host.change` event, and read again after `events.resync` or when the event channel opens again (including its first open, so an event between the first read and the channel opening is not lost). `parseHostDescriptor` checks every descriptor against the contract; one that fails is never shown. Until the descriptor arrives it shows `<fs-states>` loading, and when it cannot be read the error, offline or denied state with Retry, never a guessed host. It shows the ADR 032 Decision 6 statement for the host type from the label table (`hostDesktop`, `hostServer` with `{operator}` filled from `operator_name` or the `hostOperatorUnnamed` label, plus `hostServerAtRest`, `hostBrowser`), then, apart from it, the hosted backup and sync service: the not-enrolled state, or Backup and Sync each On or Off, the service host and the ciphertext-only statement (`hostDesktopBackup` is added on a desktop with backup or sync on). It is one focusable, named `role="group"` with a polite live region. `describeHost(descriptor, labels)` is the pure wording rule another view can reuse. `refresh()` reopens the event subscription with the token known now and reads again; the shell calls it from `refreshHealth()` after a sign-in. Once the event channel itself reconnects automatically (a later client task), the indicator needs no change.
- **`<fs-nav-tree>`** takes `{ labels, source, store?, href? }`. The `NavigationSource` answers the navigation listing one level at a time, in the shape of the API's `GET /api/v1/navigation`: `list(parent, cursor, limit)` returns `{ nodes, ancestors, next_cursor }`, each node an opaque `node` handle with `kind` (`repository`, `library`, `collection`, `virtual_collection`, `folder`), `name` (or `null`, shown as the kind's label) and `has_children`, and an unknown node is refused `not_found`; `subscribe(listener)` is optional. `MemoryNavigationSource` answers from a fixture tree; the live source over `FileSquidClient` arrives with Task UI-F2.1b. The tree reads the repository, its first 64 libraries and their children at start (a library whose children cannot be read is logged and contributes nothing to the sections until the next load), and a deeper node's children when it opens (every page, at most 50 pages per level); an answer that arrives after a newer load is dropped, and a pin whose read fails is read again only after the next load or choice change. `MemoryNavigationSource` issues opaque cursors and refuses a limit outside 1 to 1,000 or a cursor it did not issue with `invalid_request`, as the API does. It groups a library's children into Collections, Virtual collections and Folders sections, after a Pinned section when something is pinned; with several libraries each section lists, as parents, the libraries that have children of its kind. Every section has a heading and Move up and Move down buttons (`aria-disabled` at the ends; Alt+Shift+Up and Alt+Shift+Down anywhere in the section). A section with items adds a WAI-ARIA treeview (one Tab stop; Up, Down, Home, End, Right, Left, `*` and type-ahead; Right and Left swap in RTL) and a Pin toggle (`aria-pressed`) for the tree's active item. A section without items shows a "Nothing here yet" note and no tree or Pin toggle. A pin the source answers `not_found` for shows as unavailable with a Remove button, so a Pinned section that holds only such pins shows that list and no tree or Pin toggle. With no library and no pins, `<fs-states>` shows the empty state.
- **`<fs-breadcrumb>`** shows `items` as a `nav` named by `navigationBreadcrumb`, with `aria-current="page"` on the last link; `breadcrumbItems(ancestors, labels)` builds them from a listing's `ancestors`. It is hidden while the trail is empty.
- **The shell's `navigation` slot.** `<fs-nav-tree slot="navigation">` inside `<fs-app-shell>` replaces the static destination list in the `nav` landmark; the Monitor link stays after it.

**Navigation choices** are kept on the device in Web Storage under `filesquid.navigation` until per-user preferences reach the API: `{ "version": 1, "pinned": [{ "node", "kind" }], "order": [section ids], "collapsed": [node handles] }`, with at most 100 pins and 1,000 collapsed nodes and handles of at most 512 bytes. It holds no name, path, token or content handle. Repositories and libraries are open unless collapsed (kept); deeper nodes open for the session only. A stored value that does not parse, has another version, has another field or breaks a bound is ignored with a console message; if the browser refuses storage, the choices apply until the page closes and the console says why. Every tree on a page shares one store (`sharedNavigationStore(window)`) unless it is given its own; the store follows `storage` events, so a save in another tab is taken up before this tab saves again.

```ts
const source = new MemoryNavigationSource(fixtureTree); // UI-F2.1b: a source over FileSquidClient
const tree = document.createElement("fs-nav-tree") as FsNavTree;
tree.slot = "navigation";
tree.configure({ labels: distribution.labels, source });
shell.append(tree);
```

`npm test` covers the model, store, source, loader, key rules and elements in jsdom; `npm run test:e2e` checks them in the built UI ([`tests/e2e/navigation.spec.ts`](./tests/e2e/navigation.spec.ts)): accessibility tree, treeview keys, persistence across a reload, refused storage, the five states in light and dark, focus rings, motion, forced colours and reflow.

## Host adapter and client interface

Views reach FileSquid only through `FileSquidClient` ([UI-S-012](./resources/srs.md), [ADR 030](../../../resources/architecture/030-filesquid-api-and-transports.md)); they never call `fetch`, `WebSocket` or Tauri APIs. `createHostClient()` picks the adapter at run time with `selectHostAdapter(detectHost(window))`:

| Host                          | Adapter          | API base URL                                                                                 | Events                        |
| ----------------------------- | ---------------- | -------------------------------------------------------------------------------------------- | ----------------------------- |
| `filesquid-server`            | `server-http`    | same origin (`/api/v1/...`)                                                                  | WebSocket `/api/v1/events`    |
| `filesquid-desktop` (Tauri 2) | `desktop-scheme` | `filesquid://localhost` (macOS, iOS, Linux); `http://filesquid.localhost` (Windows, Android) | Tauri event `filesquid-event` |

The encrypted browser host (UI-F5.1) will add a third adapter behind the same interface.

| Method                                               | Use                                                                                                                                                    |
| ---------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `request(path, { method, body, signal })`            | JSON routes; returns the parsed body (`undefined` for 204)                                                                                             |
| `read(path, { range, signal })`                      | Bytes; an optional inclusive `range` sends `Range: bytes=start-end` (or `bytes=start-` when `end` is omitted) and reports `partial` and `contentRange` |
| `command(path, envelope, input, { method, signal })` | Mutations; JSON body `{ envelope, input }`; returns a `CommandOutcome`                                                                                 |
| `upload(path, file, envelope, { filename, signal })` | One file as `multipart/form-data` with `envelope` and `file` parts; returns a `CommandOutcome`                                                         |
| `subscribe(listener)`                                | Push events; returns the function that ends the subscription                                                                                           |

Every command carries a `CommandEnvelope` built with `commandEnvelope({ scope, expectedRevision, operationId? })` ([CORE-S-005](../crates/core/resources/srs.md)). The operation id is the idempotency key: to retry, send the same envelope again. The outcome is `committed`, `pending`, `conflict` or `error` (with a `PortError`); HTTP 409 becomes `conflict`, any other non-2xx answer is an `ApiError`, a 2xx answer that is not a `CommandOutcome` for that operation throws a `TypeError`, and a network failure rejects with the transport's own error. Only `isCommitted(outcome)` is completion evidence; a pending outcome or UI state never is. No file route exists yet: the envelope binding above is the client's side of the routes that API-F1.3 adds, and changes with that contract if it differs.

## Authenticating to the FileSquid API

The API is deny-by-default ([ADR 030](../../../resources/architecture/030-filesquid-api-and-transports.md) Decision 5, [API-S-004](../crates/api/resources/srs.md)). With a known token, every HTTP request from `FileSquidClient` carries `Authorization: Bearer <token>`; without one it sends no credential and the API answers `missing_credentials`. The event channel accepts the same header on WebSocket upgrades from clients that can set headers. A browser cannot, so `FileSquidClient` offers the ordered subprotocols `filesquid.events.v1` then `filesquid.bearer.<token>` instead. Only the liveness probe `GET /api/v1/health/live` is public.

**Desktop host.** Nothing to configure. `filesquid-desktop` generates a new token at every launch and defines it as `window.__FILESQUID_LAUNCH_TOKEN__` for its own pages only. `createHostClient()` reads it on every request.

**Server host.** Until server accounts arrive, the token is the administrator token ([API-S-016](../crates/api/resources/srs.md); details in the root [README](../../../README.md)):

- `FILESQUID_ADMIN_TOKEN`, on every platform. Surrounding whitespace is trimmed and a blank value counts as unset; any other value must be exactly 64 hexadecimal characters (256 bits), or the server refuses to start. Generate one with `openssl rand -hex 32`.
- Otherwise, on Linux and macOS only, the token the server generates at first start into `admin-token` in its data directory (`FILESQUID_DATA_DIR`, owner-only permissions). On Windows the server writes no token file and refuses to start until `FILESQUID_ADMIN_TOKEN` is set.

Pass the token through client configuration:

```ts
import { FileSquidClient, createHostClient, memoryTokenStore } from "@filesquid/ui";

// A fixed token supplied by the operator:
const client = new FileSquidClient({ baseUrl: "", token: operatorToken });

// Or a token typed in at run time, as the default app does through the Monitor's token form:
const tokens = memoryTokenStore();
const hostClient = createHostClient(window, () => tokens.get());
```

`token` is either a string or a function the client asks on every request, so a token entered later is used at once. On the desktop the launch token takes precedence. In the default FileSquid and ImageSquid apps, open **Monitor** (`#/monitor`) and enter the administrator token; the shell's API status reloads with it.

The token is kept in memory only: never in localStorage, sessionStorage, a cookie or the URL, and never logged, rendered or put in an error message. Reloading the page forgets it. A supplied token that is empty, or has characters that neither a header nor a subprotocol can carry, is refused before anything is sent. `memoryTokenStore` returns no token for an empty entry, so a blank sign-in reads as missing credentials.

**Errors.** A refused call rejects with `ApiError`, which carries the contract's `status`, `code`, `retryable` and, for authorisation denials, `reason`:

| HTTP         | `code`         | `reason`              | Label key shown by the UI |
| ------------ | -------------- | --------------------- | ------------------------- |
| 401          | `unauthorized` | `missing_credentials` | `authMissingCredentials`  |
| 403          | `forbidden`    | `invalid_credentials` | `authInvalidCredentials`  |
| 403          | `forbidden`    | `not_permitted`       | `authNotPermitted`        |
| 0 (not sent) | `forbidden`    | `invalid_credentials` | `authInvalidCredentials`  |

`denialLabel(error, labels)` maps an error to the active label table, so each distribution words the three cases itself.
