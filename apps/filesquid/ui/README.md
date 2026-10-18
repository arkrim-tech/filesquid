# @filesquid/ui

FileSquid's default distribution and the component library every distribution builds on: the `FileSquidClient` API client and host adapter, vanilla TypeScript custom elements (`<fs-app-shell>`, `<fs-monitor>`), design tokens and label tables ([ADR 031](../../../resources/architecture/031-vanilla-typescript-ui-and-white-label-theming.md)). Requirements: [SRS](./resources/srs.md). Plan: [plan](./resources/plan.md).

## Develop and test

From `apps/filesquid/ui`, on any OS with Node.js 22.12 or later:

```sh
npm ci
npm run dev            # http://127.0.0.1:5173 (preview: 4173)
npm run build          # type-check, then the static build in dist/
npm test               # Vitest unit and jsdom UI tests; JUnit in tests/allure-results/
npm run test:e2e       # Playwright against the built UI and a real filesquid-server
npm run contract:types # regenerate src/api/schema.d.ts from crates/api/openapi.json
```

`npm run test:e2e` builds the UI and starts `filesquid-server` with Cargo, so it needs the Rust toolchain and a Playwright browser (`npx playwright install chromium`). The repository's pre-commit hook runs it when `FILESQUID_E2E=1` is set.

## Host adapter and client interface

Views reach FileSquid only through `FileSquidClient` ([UI-S-012](./resources/srs.md), [ADR 030](../../../resources/architecture/030-filesquid-api-and-transports.md)); they never call `fetch`, `WebSocket` or Tauri APIs. `createHostClient()` picks the adapter at run time with `selectHostAdapter(detectHost(window))`:

| Host                          | Adapter          | API base URL                                                                                 | Events                        |
| ----------------------------- | ---------------- | -------------------------------------------------------------------------------------------- | ----------------------------- |
| `filesquid-server`            | `server-http`    | same origin (`/api/v1/...`)                                                                  | WebSocket `/api/v1/events`    |
| `filesquid-desktop` (Tauri 2) | `desktop-scheme` | `filesquid://localhost` (macOS, iOS, Linux); `http://filesquid.localhost` (Windows, Android) | Tauri event `filesquid-event` |

The encrypted browser host (UI-F5.1) will add a third adapter behind the same interface.

| Method                                               | Use                                                                                                          |
| ---------------------------------------------------- | ------------------------------------------------------------------------------------------------------------ |
| `request(path, { method, body, signal })`            | JSON routes; returns the parsed body (`undefined` for 204)                                                   |
| `read(path, { range, signal })`                      | Bytes; an optional inclusive `range` sends `Range: bytes=start-end` and reports `partial` and `contentRange` |
| `command(path, envelope, input, { method, signal })` | Mutations; JSON body `{ envelope, input }`; returns a `CommandOutcome`                                       |
| `upload(path, file, envelope, { filename, signal })` | One file as `multipart/form-data` with `envelope` and `file` parts; returns a `CommandOutcome`               |
| `subscribe(listener)`                                | Push events; returns the function that ends the subscription                                                 |

Every command carries a `CommandEnvelope` built with `commandEnvelope({ scope, expectedRevision, operationId? })` ([CORE-S-005](../crates/core/resources/srs.md)). The operation id is the idempotency key: to retry, send the same envelope again. The outcome is `committed`, `pending`, `conflict` or `error` (with a `PortError`); HTTP 409 becomes `conflict`, and any other failure is an `ApiError`. Only `isCommitted(outcome)` is completion evidence; a pending outcome or UI state never is. No file route exists yet: the envelope binding above is the client's side of the routes that API-F1.3 adds, and changes with that contract if it differs.

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
