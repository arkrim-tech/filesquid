# @filesquid/ui

FileSquid's default distribution and the component library every distribution builds on: the `FileSquidClient` API client and host adapter, vanilla TypeScript custom elements (`<fs-app-shell>`, `<fs-monitor>`, `<fs-ocfl-findings>`, `<fs-nav-tree>`, `<fs-breadcrumb>`, `<fs-states>`, `<fs-host-indicator>`, `<fs-file-browser>`, `<fs-file-preview>`, `<fs-library-start>`, `<fs-library-detail>`, `<fs-library-repair>`), design tokens and label tables ([ADR 031](../../../resources/architecture/031-vanilla-typescript-ui-and-white-label-theming.md)). Requirements: [SRS](./resources/srs.md). Plan: [plan](./resources/plan.md).

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

- **Keyboard.** Everything is a native control (`a[href]`, `button`, `input`); nothing has a positive `tabindex`. The shell's Tab order is the skip control, the theme control (one stop; arrow keys choose), the host badge, the navigation (links, or a slotted tree's controls), then the main area.
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

## Monitor: OCFL validation findings

`<fs-monitor>` (`bindMonitorRoute(shell, { labels, client, tokens?, pollMs?, repairHref? })`, route `#/monitor`) holds an **OCFL validation** section below Health (Task UI-F2.14 in the [plan](./resources/plan.md), [API-S-022](../crates/api/resources/srs.md)). The section is its own element, `<fs-ocfl-findings>`, configured with `{ labels, client, repairHref?, online? }`, where the client implements `FindingsClient` (`adminOcflFindings()` and `startOcflValidation(request)`, which `FileSquidClient` does).

- **Reads** `GET /api/v1/admin/ocfl/findings` (`client.adminOcflFindings()`): a **Validation runs** table with one row per open library (running or idle, last run start and end, outcome, objects checked and not checked, next scheduled run; "Never" and "Not scheduled" when there is none, "Nothing open to validate" with no library open), the schedule in force (hours between runs, objects per run), an **Errors** table and, apart from it, a **Warnings** table (library, object, version, code, location). A location the API withholds (null) shows "Not shown", never a path. When the route says `truncated` (more than 1,000 open), a note says more findings are open than are listed.
- **W010.** A library with W010 warnings gets a note that the files in those objects can be read but not changed. When the findings list is truncated the count says "at least". With `repairHref(libraryId)` the note links to that library's repair offer (UI-1.1 supplies it); without it the note has no link.
- **Validate now** posts an empty `RunRequest` to `POST /api/v1/admin/ocfl/validation-runs` (`client.startOcflValidation({})`), says how many runs started and how many were refused, naming each refused library with its error code, or why the start was refused, then reads the findings again.
- **Live updates.** The Monitor calls `refresh()` on every `health.change` and on every `process.update` whose kind is `ocfl.validation`; on `events.resync` it reads health, processes, metrics, the journal page and the findings again. Refreshes are coalesced: one read in flight and at most one queued behind it, and a queued read is dropped once the element leaves the page.
- **Health reason.** The `ocfl_validation_errors` reason is worded from the `healthOcflErrors` label with its codes and number of objects.
- **Failure.** A failed read shows `<fs-states>` in this section only (denied, unavailable on a 503, offline or error); the error, offline and unavailable states offer Retry, the denied state does not (the caller's authorization must be corrected first). The rest of the Monitor stays up.

Its words are the `monitorFindings`, `healthOcflErrors`, `findings*` and `run*` keys of the label table. Tests: `tests/ui/ocfl-findings.test.ts` (Vitest DOM), `tests/e2e/preview/ocfl-findings.spec.ts` (production build with contract-shaped answers: E023 with the degraded reason, closure by `health.change`, W010 apart with health ok, a withheld location, run times, token colours in light and dark) and `tests/e2e/ocfl-findings.spec.ts` (built UI on the real `filesquid-server` and its findings and run routes).

## Navigation, breadcrumb and view states

These elements need no API route of their own (Task UI-F2.1a in the [plan](./resources/plan.md), described in [Task-UI-F2.1a-Walkthrough.md](./resources/tasks/Task-UI-F2.1a-Walkthrough.md)). Data flows down as properties and changes flow up as events ([ADR 031](../../../resources/architecture/031-vanilla-typescript-ui-and-white-label-theming.md) Decision 6); none of them fetches by itself.

- **`<fs-states>`** shows one `ViewState` instead of content: `loading` and `empty` in a polite `role="status"` region, `error`, `offline`, `denied` (with the denial reason's label) and `unavailable` (an HTTP 503: the host has not installed or started that service yet, or its storage cannot be reached right now) in a `role="alert"` region. Both regions exist from the first render and are updated in place. Error, offline and unavailable offer Retry, which dispatches a bubbling, composed `fs-retry` event. `failureState(error, navigator.onLine)` picks denied, unavailable, offline or error for a failed load.
- **`<fs-host-indicator>`** (Task UI-F2.1b, part 1; [UI-S-004 and UI-S-018](./resources/srs.md)) is the persistent host privacy indicator, mounted by `<fs-app-shell>` in its header so it shows on every view; the shell's client must therefore implement `HostClient` (`host()` and `subscribe(listener, onOpen?)`, which `FileSquidClient` does). It takes `{ labels, client }` and takes its facts only from the host descriptor (`GET /api/v1/host`, read with `client.host()`): at start, replaced by the whole descriptor of each `host.change` event, and read again after `events.resync` or when the event channel opens again (including its first open, so an event between the first read and the channel opening is not lost). `parseHostDescriptor` checks every descriptor against the closed contract (exact fields, operator name only on a server, lowercase DNS service host); one that fails is never shown. Until the descriptor arrives it shows `<fs-states>` loading, and when it cannot be read the error, offline or unavailable state with Retry, or the denied state with its reason, never a guessed host. It shows the ADR 032 Decision 6 statement for the host type from the label table (`hostDesktop`, `hostServer` with `{operator}` filled from `operator_name` or the `hostOperatorUnnamed` label, plus `hostServerAtRest`, `hostBrowser`), then, apart from it, the hosted backup and sync service: the not-enrolled state, or Backup and Sync each On or Off, the service host and the ciphertext-only statement (`hostDesktopBackup` is added on a desktop with backup or sync on). The header shows it as a compact badge (Task UI-F2.1b part 2, owner decision 2026-10-19, option 1a): an icon and a short host label (`hostBadgeDesktop` "This device", `hostBadgeServer`, `hostBadgeBrowser`; `hostBadgeLoading` and `hostBadgeUnknown` while the descriptor is loading or cannot be read) in a group named by `hostIndicator`. The badge is a button (`aria-expanded`, `aria-haspopup="dialog"`); click, Enter or Space opens a non-modal popover (`role="dialog"`, named by its title) holding the full statement and moves focus into it; Escape or the badge closes it and returns focus to the badge; a click outside or focus leaving closes it. The statement is never permanent header text. The short label is a polite live region, so a host change is announced. `describeHost(descriptor, labels)` is the pure wording rule, and `buildHostStatement(view, labels)` (styled by `HOST_STATEMENT_STYLE`) builds the full statement. The popover ends with a "Privacy settings" link (`hostPrivacySettings`) to `#/settings`. With `inline: true` the element shows the statement in place, with no badge or popover, as a named region. `refresh()` reopens the event subscription with the token known now and reads again; the shell calls it from `refreshHealth()` after a sign-in. Once the event channel itself reconnects automatically (a later client task), the indicator needs no change.
- **`<fs-settings>` and `bindSettingsRoute(shell, { labels, client })`** (Task UI-F2.1b part 2): `#/settings` (the shell's Settings link, after Monitor) shows the Settings page, headed by `settings`, with its Privacy section (`settingsPrivacy`): an inline `<fs-host-indicator>`, so the full statement follows the host descriptor exactly as the badge does. UI-F2.8 adds the other sections. `refresh()` refreshes its inline indicator; the shell's `refreshHealth()` calls it for a Settings page it holds.
- **The library flow** (Task UI-1.1; [SRS-F-001](../../../resources/srs.md#srs-f-001-independent-local-library-and-ownership), [API-S-023, API-S-025 and API-S-034](../crates/api/resources/srs.md)): `bindLibraryRoute(shell, { labels, source, registry, navigate?, online? })` mounts one page for the hash. The shell's "Create or open a library" link (`libraryNavLink`) leads to `#/library`, `<fs-library-start>`, with three choices: create a library, open one, or recover from a copy. Each asks the host for a folder through `POST /api/v1/libraries/location-choices` (purpose `create`, `open`, or `open` for a recovery; the desktop shows its native dialog, and `filesquid-server` answers `503 unavailable` until it has a location registry), receives only an opaque location ID, passes it to the library route as `location_id`, and goes to `#/library/<library_id>`, `<fs-library-detail>`, which shows the library's ID, its OCFL version and object count (never a path) and offers "Copy library to…" (`copy_destination`, then `destination_location_id` to `snapshot-copies`, with the report and each copied object's validation codes) and "Rebuild catalog from files" (recovery rebuilds after opening; nothing is repaired). A cancelled choice sends nothing and changes nothing; a `503 unavailable` answer is the `<fs-states>` unavailable state with its reason (`libraryChooserUnavailable`) and a Retry, and the three choices stay on the page. Retry resumes from the step that failed: a folder already chosen is not asked for again (the host would refuse a second registration of an open folder as in use), a recovered copy already opened is only rebuilt again, and a failed copy goes again to the destination already chosen; a choice or the Copy button always starts afresh. When objects have no version inventory (W010) the library page says their files can be read but not changed and links to `#/library/<id>/repair`, `<fs-library-repair>`, the address the app passes to `bindMonitorRoute` as `repairHref` (`libraryRepairHref`). The repair offer lists the objects and opens a modal consent dialog that names them and says one copy of the root inventory and its sidecar is added to each object's latest version directory and nothing else changes; only Confirm calls `POST /api/v1/libraries/repairs` with the consent for exactly those objects (`consentRequest`), Cancel and Escape send nothing, and the result is listed per object. The `LibrarySource` interface (`ClientLibrarySource` over `FileSquidClient`) and the `LibraryRegistry` of libraries seen in the session are what later pages such as the import UI use; the open library's ID is the one in the route. Words: the `library*` and `repair*` label keys. Tests: `tests/unit/library-*.test.ts`, `tests/ui/library-*.test.ts` and `tests/e2e/preview/library.spec.ts` (production build with contract-shaped answers: location ID passed on, cancel, 503, consent confirm, cancel and refusal, the Monitor link, token colours in light and dark).
- **`<fs-nav-tree>`** takes `{ labels, source, store?, href? }`. The `NavigationSource` answers the navigation listing one level at a time, in the shape of the API's `GET /api/v1/navigation`: `list(parent, cursor, limit)` returns `{ nodes, ancestors, next_cursor }`, each node an opaque `node` handle with `kind` (`repository`, `library`, `collection`, `virtual_collection`, `folder`), `name` (or `null`, shown as the kind's label) and `has_children`, and an unknown node is refused `not_found`; `subscribe(listener)` is optional. `MemoryNavigationSource` answers from a fixture tree. `ClientNavigationSource` (Task UI-F2.1b part 2) is the live source over `FileSquidClient.navigation({ node, cursor, limit })` (`GET /api/v1/navigation`, [API-S-021](../crates/api/resources/srs.md)) with the generated contract types: `parseNavigationPage` checks every answer (handles of 1 to 512 bytes, a known kind, a `null` name only for a repository, boolean `has_children`, at most 1,000 nodes, a non-empty or `null` cursor) and one that fails is refused as an error, never shown; API errors pass through. It shares one event subscription between its listeners: `navigation.change` becomes the named `parents` (checked by `parseNavigationChange`: exactly one of 1 to 64 handles and `all`), and `all`, `events.resync`, each channel open and a payload that breaks the contract read the whole visible tree again; `reconnect()` opens a new channel after a token is entered. On a `parents` change the tree reads only the loaded levels named and the shown level holding each named node, renames pins in them, and closes a level answered `not_found`; the repository, `all` or a change before the first answer reads everything. A 503 (the desktop host until DE-1.1 installs its navigation service, and `filesquid-server` until it does) shows the unavailable state with Retry. The tree reads the repository, its first 64 libraries and their children at start (a library whose children cannot be read is logged and contributes nothing to the sections until the next load), and a deeper node's children when it opens (every page, at most 50 pages per level); an answer that arrives after a newer load is dropped, and a pin whose read fails is read again only after the next load or choice change. `MemoryNavigationSource` issues opaque cursors and refuses a limit outside 1 to 1,000 or a cursor it did not issue with `invalid_request`, as the API does. It groups a library's children into Collections, Virtual collections and Folders sections, after a Pinned section when something is pinned; with several libraries each section lists, as parents, the libraries that have children of its kind. Every section has a heading and Move up and Move down buttons (`aria-disabled` at the ends; Alt+Shift+Up and Alt+Shift+Down anywhere in the section). A section with items adds a WAI-ARIA treeview (one Tab stop; Up, Down, Home, End, Right, Left, `*` and type-ahead; Right and Left swap in RTL) and a Pin toggle (`aria-pressed`) for the tree's active item. A section without items shows a "Nothing here yet" note and no tree or Pin toggle. A pin the source answers `not_found` for shows as unavailable with a Remove button, so a Pinned section that holds only such pins shows that list and no tree or Pin toggle. With no library and no pins, `<fs-states>` shows the empty state.
- **`<fs-breadcrumb>`** shows `items` as a `nav` named by `navigationBreadcrumb`, with `aria-current="page"` on the last link; `breadcrumbItems(ancestors, labels)` builds them from a listing's `ancestors`. It is hidden while the trail is empty.
- **The shell's `navigation` slot.** `<fs-nav-tree slot="navigation">` inside `<fs-app-shell>` replaces the static destination list in the `nav` landmark; the Monitor link stays after it. The default distribution (`src/app/main.ts`) slots the live tree.
- **`<fs-navigation-view>` and `bindNavigationRoute(shell, { labels, source, tree })`** (Task UI-F2.1b part 2): `#/navigation/<handle>` (the tree's links; `navigationHref`) mounts the view in the shell's main area and marks the tree's current node. The view reads one listing of the destination (`list(handle, null, 1)`) and fills its `<fs-breadcrumb>` from `ancestors`, repository first and the destination last as the current page, headed by the destination's name; it reads again when a change names a node of the trail. It shows loading, a gone destination for `not_found`, and the unavailable, error, offline or denied state, never a guessed trail. The destination's files cannot be listed yet: the file routes list one OCFL object, and a listing by navigation node is an API addition still to come, so the view says so and `#/files/...` stays the way to the file browser.

**Navigation choices** are kept on the device in Web Storage under `filesquid.navigation` until per-user preferences reach the API: `{ "version": 1, "pinned": [{ "node", "kind" }], "order": [section ids], "collapsed": [node handles] }`, with at most 100 pins and 1,000 collapsed nodes and handles of at most 512 bytes. It holds no name, path, token or content handle. Several `filesquid-server` users who share one browser profile share these choices until then; this is accepted ([UI-S-017](./resources/srs.md)). Repositories and libraries are open unless collapsed (kept); deeper nodes open for the session only. A stored value that does not parse, has another version, has another field or breaks a bound is ignored with a console message; if the browser refuses storage, the choices apply until the page closes and the console says why. Every tree on a page shares one store (`sharedNavigationStore(window)`) unless it is given its own; the store follows `storage` events, so a save in another tab is taken up before this tab saves again.

```ts
const source = new ClientNavigationSource(client); // or new MemoryNavigationSource(fixtureTree)
const tree = document.createElement("fs-nav-tree") as FsNavTree;
tree.slot = "navigation";
tree.configure({ labels: distribution.labels, source });
shell.append(tree);
bindNavigationRoute(shell, { labels: distribution.labels, source, tree });
```

`npm test` covers the model, store, source, loader, key rules and elements in jsdom; `npm run test:e2e` checks them in the built UI ([`tests/e2e/navigation.spec.ts`](./tests/e2e/navigation.spec.ts)): accessibility tree, treeview keys, persistence across a reload, refused storage, the five states in light and dark, focus rings, motion, forced colours and reflow; [`tests/e2e/preview/navigation.spec.ts`](./tests/e2e/preview/navigation.spec.ts) checks the live source against the production build with the listing answered by the test: paging through cursors, the breadcrumb from `ancestors`, a `navigation.change` without a reload, the 503 state and token colours in light and dark.

## File browser

`<fs-file-browser>` (Task UI-F2.2; [UI-S-002, UI-S-019](./resources/srs.md); walkthrough: [Task-UI-F2.2-Walkthrough.md](./resources/tasks/Task-UI-F2.2-Walkthrough.md)) is the Drive-style browser of one OCFL object: a list (default) or grid of folders and files with name, size, type hint, modified and version, a breadcrumb of folders, a viewer with a preview, and upload, new folder, rename or move, remove and Undo. Every row, tile, heading and message shows the file's **display name** from the API; the stored name is never rendered. It reads and writes only through a `FileSource` (`ClientFileSource` over `FileSquidClient`; `MemoryFileSource` for tests and demos), so it never fetches by itself.

```ts
defineElements();
const browser = document.createElement("fs-file-browser") as FsFileBrowser;
browser.configure({ labels, source: new ClientFileSource(client) });
browser.location = { library_id, object_id, prefix: "" }; // loads the folder
browser.registerPanel("details", () => myDetailsPanel()); // sidebar slot, gets `fileRef`
browser.registerPreviewRenderer(myRenderer); // tried before the built-ins
shell.append(browser);
```

- **Route.** `bindFilesRoute(shell, { labels, source })` shows the browser for `#/files/<library>/<object>/<folder>/...` (each segment percent-encoded; see `locationHash`) and follows the Back button. Until the API lists files by navigation node, this route is the way to reach the browser.
- **Writes.** Each is one API call that commits one OCFL version: `PUT files/upload` (one call per file, at most 64 MiB, refused before sending beyond that; the query carries the library and the file's facts, never a logical path: `name`, `size`, `name_normalization`, and `media_type` and `last_modified` when the browser reports them in a form the API takes, and the API stores the file in its own object `files/<file-id>`, [API-S-026, API-S-035](../crates/api/resources/srs.md); the API gives an uploaded file no home collection or folder yet, so it does not appear in the folder shown or the navigation tree after the upload), `POST files/folders` (the folder stays pending, marked, until a file is added beneath it), `POST files/renames` (rename a file in its own object: sets only its display name as a new version of its metadata record, [API-S-027](../crates/api/resources/srs.md); the form offers no folder, and a refusal shows the label for its code: `invalid_request`, `not_found`, `conflict`, `unavailable`), `POST files/moves` (a folder's rename, and a real move of a file in any other object; typing the name already shown keeps the stored name; the API refuses a move from or to `original` or below it in a file object), `POST files/removals` and `POST files/restorations` (Undo restores from the version before the removal). There is no delete-forever control. A refusal shows the label for its reason and changes nothing.
- **Preview.** The preview reads the file's media type from `GET files/details`. Image, video and audio files play natively through a **content handle** minted by `FileSquidClient.mintContentHandle` ([UI-S-014](./resources/srs.md)): the handle URL is only ever a media element's `src`, never in the address, history, storage, a log, text or a link. On a media error one new handle is minted and playback resumes at the same position and paused state; a second failure shows the error. Text is read with one ranged read of the first 64 KiB. Anything else says there is no preview.
- **Panel slots.** A panel is an element with a `fileRef` property (`FileRef`: `library_id`, `object_id`, `version`, `path`, `display_name`, or null when no file is open). `registerPanel(id, create)` mounts it in the sidebar in registration order; panels get no reference to each other (ADR 028 Decision 6).
- **Events** (bubbling, composed): `fs-location-change` (`detail.location`), `fs-file-open` (`detail.file`, null on close), `fs-files-change` (`detail.operation`, `detail.commit`).
- **Large folders.** Only the rows near the viewport are in the DOM (5 rows of overscan; rows are `3rem` tall in the list and `9rem` in the grid, so text scaling holds), rows are keyed by logical path and keep their element and focus, and pages of 200 entries load as the window nears the end, up to 500 pages.
- **Keyboard.** The listing is one Tab stop; ArrowUp, ArrowDown, Home, End, PageUp and PageDown move the active row (the sideways arrows too in the grid), Enter opens it, F2 renames and Delete removes it. Rename and Remove in the toolbar act on the row that last had focus, or on the open file.

The element lists one object's logical tree because that is what the file routes (API-F1.3) answer; the navigation node handles (UI-F2.1b) name collections and folders that span many objects, so opening the browser from the tree waits for a file listing by navigation node (the library-level file views of API-S-019), which UI-F2.8 builds on.

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
| `uploadFile(query, body, { signal })`                | `PUT /api/v1/files/upload` with the contract's query (`FileUploadQuery` without `size`, which is the body's size); returns the `UploadCommit`          |
| `subscribe(listener)`                                | Push events; returns the function that ends the subscription                                                                                           |

Every command carries a `CommandEnvelope` built with `commandEnvelope({ scope, expectedRevision, operationId? })` ([CORE-S-005](../crates/core/resources/srs.md)). The operation id is the idempotency key: to retry, send the same envelope again. The outcome is `committed`, `pending`, `conflict` or `error` (with a `PortError`); HTTP 409 becomes `conflict`, any other non-2xx answer is an `ApiError`, a 2xx answer that is not a `CommandOutcome` for that operation throws a `TypeError`, and a network failure rejects with the transport's own error. Only `isCommitted(outcome)` is completion evidence; a pending outcome or UI state never is. The file routes of API-F1.3 do not take an envelope yet: `uploadFile` sends raw bytes with the facts in the query and answers an `UploadCommit` (an `ApiError` on refusal, `conflict` included), and the other file writes are plain JSON calls through `ClientFileSource`. The envelope binding above is the client's side of the contract-wide envelope change of API-F2.13, and changes with that contract if it differs.

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
