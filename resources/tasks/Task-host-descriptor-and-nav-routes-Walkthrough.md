# Task host-descriptor-and-nav-routes Walkthrough: host descriptor, navigation listing and the UI-F2.1 split

Documentation-only task on branch `docs/host-descriptor-and-nav-routes` (owner-approved 2026-10-18). TDD-ENFORCE: Not applicable - documentation update; [API-F1.10](../../apps/filesquid/crates/api/resources/plan.md), [API-F1.11](../../apps/filesquid/crates/api/resources/plan.md), [UI-F2.1a](../../apps/filesquid/ui/resources/plan.md) and [UI-F2.1b](../../apps/filesquid/ui/resources/plan.md) implement and test it.

## The gap

UI-F2.1 (navigation, breadcrumb, states and host privacy indicator) needed API data that no plan task delivered:

1. The host type, operator name and hosted backup and sync state that [UI-S-004](../../apps/filesquid/ui/resources/srs.md) and [ADR 032](../architecture/032-hosting-and-privacy-model.md) Decision 7 say must come from the API. `HealthStatus` carries only `schema_version`, `status` and `product`.
2. A route that lists repositories, libraries, collections, virtual collections and nested folders for the tree and breadcrumb. CL-0.2 (PR #33) defines the library queries but, as its own status line says, no route.
3. A home for pinned destinations, section order and collapse state before per-user preferences (API-F2.2) exist.

## API-S-020: Host descriptor

| Property | Value |
|---|---|
| Route and action | `GET /api/v1/host`, action `host.read`, granted to every accepted credential; not public |
| Fields | `schema_version` `1`; `host_type` `desktop`, `server` or `browser`; `operator_name`; `hosted_service` (`enrolled`, `backup_enabled`, `sync_enabled`, `service_host`) |
| Host type source | Supplied by the host when it builds the router; never derived from origin, scheme, user agent or headers |
| Operator name | `null` on desktop and browser; on `filesquid-server` from `FILESQUID_OPERATOR_NAME`, trimmed, 1 to 256 characters, no control characters |
| Unnamed operator | Server starts, reports `null`, writes one `server.operator_unnamed` warning to stdout and the journal at every start |
| Invalid operator name | Too long or a control character: the server refuses to start; the error names the variable and omits the value |
| Not enrolled | `enrolled`, `backup_enabled`, `sync_enabled` `false`, `service_host` `null`; the default until API-F4.1 |
| Enrolled | `service_host` is a lowercase DNS host name with no scheme, port, path, user information or query; backup and sync reported separately |
| Never in the descriptor | Token, content handle, key or key identifier, account or device identifier, e-mail address, quota, object count, size, timestamp, path, file name |
| Event | `host.change`, payload the whole new descriptor, to every authenticated subscriber regardless of role; reread after `events.resync` or a reconnect |
| Journal | Reads are not journaled; redaction tests for the answer, the event and `server.operator_unnamed` |

The backup and sync part fits ADR 032's ciphertext-only rule: the descriptor reports only whether the host is enrolled, which guarantees are on, and the service's host name. Nothing in it needs the hosted service to read private content, and it carries no account, device or key detail. No ADR amendment: exposing the host type and operator is already Decision 7, and Decision 4 is unchanged.

## API-S-021: Navigation listing

| Property | Value |
|---|---|
| Route and action | `GET /api/v1/navigation`, action `navigation.read` |
| Root | Without `node`: the connected host's one repository |
| Children | Repository: libraries. Library: collections, then virtual collections, then folders implied by its unfiled files. Collection: folders implied by its files. Folder: subfolders in the same scope. Virtual collection: none. Files: never |
| Folders | Implied by committed logical paths only; none for hidden or deleted files only; UI-pending empty folders do not exist here |
| Node | Opaque `node` handle at most 512 bytes (no filesystem path, storage key, OCFL content path, SQL or token), `kind`, `name`, `has_children` |
| Breadcrumb | `ancestors` from the repository down to and including the requested node |
| Paging | `limit` 1 to 1,000, default 200; opaque cursor; stable order by the fixed kind order `library`, `collection`, `virtual_collection`, `folder`, then catalog order or folder name in Unicode code-point order, then handle; `invalid_request` for a malformed cursor or limit |
| Permissions | Filter before paging; `has_children` counts readable children only; unknown and unreadable nodes get the same `not_found` |
| One meaning | Libraries, collections and virtual collections from the API-S-019 queries; folders from the API-F1.3 folder listing |
| Event | `navigation.change` with `parents` (1 to 64 handles) or `all` `true`, filtered per subscriber |

Assigned to a new follow-up row, API-F1.11, rather than to CL-0.2 or API-F1.3: CL-0.2 is domain meaning with no routes (PR #33 says so and is not edited), and API-F1.3 sits in Task FS-1.2, which must not wait on Task 0.10. The route needs both.

## UI-S-017: Navigation preferences

Owner decision: keep pinned destinations, section order and collapse state on the device in Web Storage, as `filesquid.theme` is (UI-S-015), and move them to API-F2.2 per-user preferences when that lands. The requirement is written without a timeline ([decoupled-execution.md](../../.agents/rules/decoupled-execution.md)): on a host whose API serves per-user preferences, keep them there; otherwise on the device. The plan carries the timing: UI-F2.1a stores them on the device and UI-F2.8 (with API-F2.2) moves them.

| Property | Value |
|---|---|
| Device key | `filesquid.navigation`, JSON with `version` `1` |
| Bounds | At most 100 pins and 1,000 collapsed node handles |
| Content | Opaque node handles and kinds, section order, collapsed handles; never a name, title, logical path, token or content handle |
| Refused storage | Choices apply until the page closes; the console says why |
| Bad value | Unparsable, another version or over a bound: ignored with a console message, defaults used |
| Missing node | A pin answered `not_found` shows as unavailable with a remove action |
| Move to the API | First run against an API with per-user preferences copies the value once, then removes the device key |

## UI-S-018: Host privacy indicator data

The indicator reads `GET /api/v1/host` at start, follows `host.change`, rereads after `events.resync` or a reconnect, shows loading and error states and never a guessed host. It uses the ADR 032 Decision 6 statements from the label table (server: the operator's name or an unnamed-operator label, plus the at-rest statement of SRS-F-057), shows the hosted service apart from the host, shows exactly "not enrolled" until enrolment, then backup and sync separately, and never presents sync as backup. UI-S-004 is unchanged.

## Plan changes

| Task | Change | Dependencies |
|---|---|---|
| API-F1.10 Host descriptor route and change event (new, 6 h) | Route, event, host injection on desktop and server, not-enrolled default | API-F1.2, API-F1.7 (both done) |
| API-F1.11 Navigation listing route (new, 10 h) | Follow-up to CL-0.2 | Task CL-0.2, API-F1.3, API-F1.7 |
| UI-F2.1a States, and the navigation tree and breadcrumb elements (12 h) | States element; tree and breadcrumb over a typed interface with fixtures; device-kept preferences | UI-0.1, UI-0.2, UI-0.5 (all done) |
| UI-F2.1b Host privacy indicator and live navigation data (8 h) | Indicator from the descriptor; live tree and breadcrumb | UI-F2.1a, API-F1.10; API-F1.11 gates only the live tree and breadcrumb |
| API-F2.2 | Per-user preferences include the navigation choices | unchanged |
| UI-F2.8 | Moves the navigation choices from the device to API-F2.2 | unchanged |
| API-F4.1 | Replaces the not-enrolled default with real enrolment state and `host.change` events | unchanged |
| API-F5.1 | The browser host supplies `browser` | unchanged |
| UI-F2.2, UI-1.1, UI-F2.7 | Depend on UI-F2.1a | |
| UI-I1.1 | Depends on UI-F2.1b | |
| Task FS-2.1 | Dispatches UI-0.1, UI-0.2, UI-0.5, UI-F2.1a, API-F1.10, API-F1.11, UI-F2.1b; 48 h; also waits on Task FS-1.2 and CL-0.2 for API-F1.11 and the live tree only | |

The checklist moves Task FS-2.1 after Task 0.10, since it now finishes after FS-1.2 and CL-0.2. The MVP grows by 16 hours (762 to 778; phase F2 288 to 304; total 2,601 to 2,617).

## Secondary SRS mapping

- **API-S-020: Host descriptor: host type, operator name and hosted backup and sync state, with a change event.** New row.
- **API-S-021: Navigation listing: one level of the tree per call, ancestors for the breadcrumb, permission-filtered paging.** New row.
- **API-S-011: The API MUST expose one paged view operation, per-user UI preferences and storage use.** Per-user preferences now name the navigation choices.
- **UI-S-017: Navigation preferences persist in the API's per-user preferences, or on the device in Web Storage.** New row.
- **UI-S-018: The host privacy indicator takes its facts only from the host descriptor.** New row.
- **SRS-F-004: Collections, folders and navigation.** New bullets for the navigation listing and preference persistence; tests listed.
- **SRS-F-057: Desktop and server hosts.** New bullet for the host descriptor; tests listed.

New Gherkin (scenarios skip until the implementing tasks add steps): [`host_descriptor_API-S-020.feature`](../../apps/filesquid/crates/api/tests/features/host_descriptor_API-S-020.feature), [`navigation_listing_API-S-021.feature`](../../apps/filesquid/crates/api/tests/features/navigation_listing_API-S-021.feature), [`host_privacy_indicator_UI-S-018.feature`](../../apps/filesquid/ui/tests/features/host_privacy_indicator_UI-S-018.feature), [`navigation_preferences_UI-S-017.feature`](../../apps/filesquid/ui/tests/features/navigation_preferences_UI-S-017.feature).

## Conflict check

No contradiction found between the PRD (F-004, F-057, F-059), the SRS, ADRs 030, 032 and 033, and the plans. PRD F-004 requires the navigation choices to persist across restarts, which device storage satisfies. The PRD needs no change; no ADR changes. ID coordination: PR #33 adds API-S-019, so the new rows start at API-S-020; both pull requests append to the API SRS table, so the second to merge resolves a one-line textual conflict.

## Verification

```text
$ bash scripts/verify_docs.sh
✅ ADR structure (ISO 42010) passed.
FileSquid: 260 vision IDs, 260 allocated, 130 allocation rows
ImageSquid: 29 vision IDs, 29 allocated, 19 allocation rows
OK: every vision ID is allocated
✅ PRD allocation passed.
OK: every requirement is accounted for in a plan task
✅ Plan traceability passed.
checked 182 Markdown files: 0 broken link(s)
✅ Relative links and anchors passed.
checked 162 documents: 0 link(s) cross the FileSquid/ImageSquid boundary
✅ FileSquid and ImageSquid document boundary passed.
✅ Task cleanup passed.
✅ Documentation validation passed.

$ bash scripts/verify_srs_traceability.sh
Starting PRD/SRS Traceability Validation...
✅ Traceability Validation passed: 95 PRD requirements, 82 resolved SRS references.
```

`bash scripts/pre-commit.sh` was run before each push; the hook output is in the pull request description.
