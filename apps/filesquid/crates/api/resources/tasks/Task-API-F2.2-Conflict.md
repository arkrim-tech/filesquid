# Task API-F2.2 Conflict Artifact: the file-write contract (decided)

Raised 2026-10-20 during [Task API-F2.2](../plan.md), per [sync-logic.md](../../../../../../.agents/rules/sync-logic.md). The file-write routes of API-F1.3 and API-F2.5 take plain JSON bodies (upload: raw bytes) and answer a `FileCommit`. [UI-S-012](../../../../ui/resources/srs.md) and the contract's `info.description` require a `CommandEnvelope` and a `CommandOutcome` with operation-ID idempotency.

**Owner decision, 2026-10-20: follow the SRS.** Envelopes and `CommandOutcome` win. The change is made later, contract-wide, as a separate API task. Until then API-F2.2 keeps the plain bodies and adds no new write route.
