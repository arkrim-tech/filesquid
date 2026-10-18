# Task API-F1.10 Plan: Host descriptor route and change event

Module task [API-F1.10](../plan.md) of global [Task FS-2.1](../../../../../../resources/implementation-plan.md). Branch `feat/api-f1.10-host-descriptor` from `main` at `ab52290`. Dependencies API-F1.2 and API-F1.7 are done.

Requirement: [API-S-020](../srs.md) (host descriptor), with [API-S-003](../srs.md), [API-S-004](../srs.md), [API-S-015](../srs.md), [API-S-017](../srs.md). Parents: [F-057](../../../../../../resources/prd.md#f-057-desktop-and-server-hosts), [SRS-F-057](../../../../../../resources/srs.md#srs-f-057-desktop-and-server-hosts), [F-059](../../../../../../resources/prd.md#f-059-hosted-encrypted-backup-and-sync-service), [SRS-F-059](../../../../../../resources/srs.md#srs-f-059-hosted-encrypted-backup-and-sync-service), [F-061](../../../../../../resources/prd.md#f-061-activity-journal-and-live-health-monitoring), [ADR 032](../../../../../../resources/architecture/032-hosting-and-privacy-model.md) Decisions 4, 6 and 7. Gherkin: [`host_descriptor_API-S-020.feature`](../../tests/features/host_descriptor_API-S-020.feature).

## Constraints copied from API-S-020

1. `GET /api/v1/host` answers a `HostDescriptor` to every caller the host authenticates.
2. Route action `host.read` is granted to every accepted credential (desktop per-launch token, server administrator credential, every signed-in server user), not to administrators only.
3. The route is not public (API-S-015): a missing credential is refused (`unauthorized`, `missing_credentials`).
4. The descriptor carries exactly `schema_version` (`1`), `host_type`, `operator_name`, `hosted_service`.
5. `host_type` is one of `desktop`, `server`, `browser`.
6. `hosted_service` carries exactly `enrolled`, `backup_enabled`, `sync_enabled`, `service_host`.
7. Each host supplies its own type when it builds the router (desktop `desktop`, server `server`, browser `browser` in API-F5.1). The API never derives it from a request (origin, scheme, user agent or any header).
8. `operator_name` is `null` on the desktop and browser hosts.
9. On the server, `operator_name` is `FILESQUID_OPERATOR_NAME`, trimmed, 1 to 256 characters, no control characters.
10. Unset or blank variable: the server still starts, reports `null`, and writes one warning naming the variable to stdout and to the journal (`server.operator_unnamed`) at every start.
11. Too long or with a control character: the server refuses to start with an error that names the variable and does not contain the value.
12. `hosted_service` comes from the host's enrolment state (an injected enrolment port).
13. `enrolled` `false` implies `backup_enabled` `false`, `sync_enabled` `false`, `service_host` `null`; a host with no enrolment state reports exactly that (the default port of this task).
14. `enrolled` `true`: `service_host` is the service's DNS host name, lowercase, no scheme, port, path, user information or query; `backup_enabled` and `sync_enabled` are separate values.
15. Privacy: neither the descriptor nor its event carries a token, content handle, key or key identifier, account or device identifier, e-mail address, quota, object count, size, timestamp, logical or filesystem path or file name.
16. Whenever any descriptor field changes, one `host.change` event whose payload is the whole new descriptor goes to every authenticated subscriber, regardless of role.
17. A client reads `GET /api/v1/host` again after `events.resync` or a reconnect (documented in the contract).
18. The contract holds the route, its action, the `HostDescriptor` schema and the `host.change` topic.
19. Reading the descriptor is a successful call and is not journaled (API-S-017).
20. Redaction tests prove the answer, a `host.change` payload and the `server.operator_unnamed` entry carry no forbidden value.

Plan description extras: regenerate `ui/src/api/schema.d.ts` (drift test passes); add `FILESQUID_OPERATOR_NAME` to the root `README.md` and the root `docker-compose.yml` server service, after running both. No ADR change (ADR 032 Decision 7).

## Approach

- `crates/api/src/host.rs` (new): `HostType`, `OperatorName` (validated, `parse` trims and maps blank to `None`), `HostedService` (private fields; `not_enrolled()`, `enrolled(host, backup, sync)` validating the DNS host name), `HostDescriptor`, `HostIdentity` (`desktop()`, `browser()`, `server(operator)`, so a non-server host cannot carry an operator), the `EnrolmentPort` trait with the `NotEnrolled` default, and `HostInfo`, which builds the descriptor and publishes `host.change` when a refresh finds a change.
- `ApiState::describe_host(identity, port)` installs it once (like the authoriser and monitor); `GET /api/v1/host` answers `503 unavailable` until a host has described itself.
- `Action::ReadHost` (`host.read`): `Authoriser::authorise` grants it to every authenticated principal before consulting the policy, so a later policy (API-F1.9) cannot withhold it.
- `events::topic::HOST_CHANGE`.
- Contract: path, action, `HostType`, `HostedService`, `HostDescriptor`, `host.change` payload, `HostNotDescribed` response; `API_VERSION` 0.6.0. The test schema checker learns `maxLength`.
- Desktop: `describe_desktop_host(state)` in `builder()`.
- Server: `ServerConfig.operator_name` parsed in `from_lookup` (error names the variable, never the value); `start_server` describes the host and emits the `server.operator_unnamed` warning once (tracing to stdout, journaled by the journal layer, or directly when no layer is attached).

## Verification

Unit (`unit_host`, `unit_auth`, `unit_contract`, `unit_events`), server integration (`integration_host`), desktop integration (`api_protocol`), cucumber steps for every scenario in the feature, UI Vitest contract drift test, then `bash scripts/pre-commit.sh`.
