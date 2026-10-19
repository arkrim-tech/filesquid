# Task DE-0.3 Plan: Intent → OCFL commit → receipt reconcile

## Design

- **Application profile in `filesquid-api` (`canonical` module):** the layout and encoding of a library's collection snapshot (one OCFL object per library, one logical file per entity) and of FileSquid file objects (one OCFL object per file); which snapshot entities a `CatalogChange` touches; committing a snapshot delta through the `filesquid-ocfl` change model (API-S-008, the only write path); reading a snapshot back for reconcile and rebuild; the dual-profile transform seam (the owner-readable local profile is the identity transform).
- **Coordinator in `filesquid-native` (`desktop`):** `SqliteCatalog::commit` records intent in its own transaction, applies the changes in an open SQLite transaction, commits the snapshot delta as one OCFL version (message names the operation), then stores receipt, journal entry and intent removal and commits SQLite. At open, each library is reconciled from its snapshot head: a catalog behind the snapshot (or missing) is rebuilt from it, receipts are recovered from version messages, and intents whose operation is published are cleared; intents with no published version stay pending.

## Constraints

1. ADR 005 sequence: intent (operation ID, expected revision) durable before any OCFL byte; OCFL version durable before the catalog update; catalog updated from the published receipt.
2. Crash before snapshot: intent pending, catalog and head unchanged; a retry under the same operation ID completes once.
3. Crash after snapshot: acknowledged collection state is never lost, even with the catalog file deleted.
4. Only the change model writes (API-S-008); W010 heads are refused, never repaired implicitly (OCFL-S-018, OCFL-S-020).
5. Writer fencing stays the engine's lease (OCFL-S-018 known gap stays named).
6. Every domain operation journaled in the same SQLite transaction as its change; reconcile journaled.
7. Native I/O only in `filesquid-native` behind `desktop`; `filesquid-api` stays free of filesystem calls.
8. Identity transform for the owner-readable profile; the encrypted profile waits for the ADR 009 cipher.
9. Bounded loops: history, snapshot entities and intents read with caps.
