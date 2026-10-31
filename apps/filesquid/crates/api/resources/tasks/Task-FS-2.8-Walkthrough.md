# Task FS-2.8 Walkthrough: PROV-O history

Global [Task FS-2.8](../../../../../../resources/implementation-plan.md#task-fs-28) ([ADR 040](../../../../../../resources/architecture/040-prov-o-history.md)). It ran as nine module slices on several branches, in the order the plan sets. Each slice's walkthrough holds its own failing runs, raw test output and SRS mapping. This walkthrough indexes them. DE-0.10, the closing desktop slice, was the last to finish (after API-F2.9 and API-F2.20, which the plan orders after it) and closed the task.

## Slices

| Slice | Title | Walkthrough |
|---|---|---|
| CO-0.11 | PROV-O history contract types | [Task-CO-0.11-Walkthrough.md](../../../core/resources/tasks/Task-CO-0.11-Walkthrough.md) |
| IO-1.5 | Version-bound logical files in the change model | [Task-IO-1.5-Walkthrough.md](../../../ocfl/resources/tasks/Task-IO-1.5-Walkthrough.md) |
| PV-0.1 | Crate, PROV-O model and profile `filesquid-prov/1` | [Task-PV-0.1-Walkthrough.md](../../../prov/resources/tasks/Task-PV-0.1-Walkthrough.md) |
| MD-0.9 | Commit options on the `ocfl` store | [Task-MD-0.9-Walkthrough.md](../../../metadata/resources/tasks/Task-MD-0.9-Walkthrough.md) |
| PV-0.2 | `ocfl` feature: records in OCFL objects and reconstruction | [Task-PV-0.2-Walkthrough.md](../../../prov/resources/tasks/Task-PV-0.2-Walkthrough.md) |
| API-F2.8 | Journal and commit linkage for PROV-O history | [Task-API-F2.8-Walkthrough.md](./Task-API-F2.8-Walkthrough.md) |
| DE-0.10 | Installation UUID, native journal columns, outbox events and coordinator commit linkage | [Task-DE-0.10-Walkthrough.md](../../../desktop/resources/tasks/Task-DE-0.10-Walkthrough.md), host wiring [Task-FS-2.8-host-wiring-Walkthrough.md](../../../desktop/resources/tasks/Task-FS-2.8-host-wiring-Walkthrough.md) |
| API-F2.9 | PROV-O records, history routes and backfill | [Task-API-F2.9-Walkthrough.md](./Task-API-F2.9-Walkthrough.md) |
| API-F2.20 | Repair consent statement names the repair's history version | [Task-API-F2.20-Walkthrough.md](./Task-API-F2.20-Walkthrough.md) |

## SRS mapping

| Requirement | Covered by |
|---|---|
| SRS-F-067 PROV-O history of every version and change | every slice above; BDD [`prov_o_history_F-067.feature`](../../tests/features/prov_o_history_F-067.feature), [`prov_o_model_F-067.feature`](../../../prov/tests/features/prov_o_model_F-067.feature), [`prov_o_history_contract_F-067.feature`](../../../core/tests/features/prov_o_history_contract_F-067.feature), [`desktop_installation_F-067.feature`](../../../desktop/tests/features/desktop_installation_F-067.feature) |
| API-S-028 PROV-O record per commit and journal linkage | API-F2.8, API-F2.9, API-F2.20; BDD [`journal_and_commit_linkage_API-S-028.feature`](../../tests/features/journal_and_commit_linkage_API-S-028.feature) |
| API-S-029 History routes and backfill | API-F2.9; BDD [`prov_o_history_records_routes_and_backfill_API-S-029.feature`](../../tests/features/prov_o_history_records_routes_and_backfill_API-S-029.feature) |
| DE-S-011 Native journal and commit linkage | DE-0.10 and the FS-2.8 host wiring |
| DE-S-012 Installation UUID | DE-0.10 |

The task's three proofs live in the slices that own them: one record per covered operation in its own version, none for a no-op or a refused call, and full history from a copied storage root to a JSON parser alone. See the API-F2.9 walkthrough and the PV-0.2 walkthrough.

## Closing run

The closing desktop slice, DE-0.10, passed `FULL=1 bash scripts/pre-commit.sh` on its head commit; the `Test scope:` line is in its pull request.

## Mocked Environment Variables

None.
