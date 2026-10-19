# Architecture Decision Record - FileSquid

015-prov-o-jsonld-and-c2pa-sdk.md

## Title

PROV-O in versioned JSON-LD plus a Rust C2PA engine (`filesquid-c2pa` per ADR 026); no graph database

## Status

Accepted

Accepted: 2026-09-16T22:24:57Z. Revised: 2026-09-22T04:34:37Z. Revised: 2026-10-15T05:24:52Z. Amended by [ADR 026](./026-filesquid-provenance-engine.md): C2PA processing and the PROV-O projection are the from-scratch `filesquid-c2pa` engine; PROV-O JSON-LD and no graph database remain.

Amended: 2026-10-19T05:18:43Z by [ADR 040](./040-prov-o-history.md) (owner decision): the PROV-O history is independent of C2PA. It is produced for every OCFL version by the stand-alone `filesquid-prov` module and FileSquid's application layer from the MVP on; the `filesquid-c2pa` engine keeps only its optional projection of C2PA actions into the same profile. The versioned JSON-LD profile is `filesquid-prov/1` with an embedded context (W3C publishes no PROV-O JSON-LD context document) and records live at `provenance/<version>.jsonld` in the object they describe.

## Context / Requirement Reference

Acquisition and retained transformations MUST be recorded ([FE-031](../product-vision.md#source-aware-clipping), [FE-036](../product-vision.md#c2pa-from-import-onward)). Paid publication uses C2PA ([FE-042](../product-vision.md#paid-c2pa-publication-watermarking-and-ai-disclosure)). A graph database is not part of the stack. The vision listed these standards but not an SDK ADR.

## Decision

- **History vocabulary:** W3C **PROV-O**, serialized as a **versioned JSON-LD** profile with pinned contexts, stored as OCFL sidecars / package files ([ADR 010](./010-ocfl-1-1-dual-profile.md)). FileSquid defines required fields; PROV-O is not a blanket truth claim. Ephemeral cache generation MUST NOT become permanent PROV-O history unless the output is retained or disclosed ([T-037](../product-vision.md#canonical-ocfl-state-and-evidence)).
- **C2PA:** the from-scratch **`filesquid-c2pa`** engine on desktop and in browser WASM ([ADR 026](./026-filesquid-provenance-engine.md)); the official `c2pa` crate is a CI interoperability oracle only. Its optional PROV-O feature projects C2PA actions to this JSON-LD profile. Import-time records and action disclosure are local. Paid signing/watermarking is an in-app third-party publisher workflow; FileSquid does not operate a CA.
- **No graph database, Elasticsearch, or vector DB** is part of this stack.

Exact JSON-LD context URLs, C2PA profile version and watermark supplier remain implementation/supplier ADRs. This ADR freezes vocabulary + SDK + “no extra database.”

## Rationale

PROV-O/JSON-LD is already the adopted history model. A Rust C2PA engine matches the Rust host ([ADR 026](./026-filesquid-provenance-engine.md) replaces the official crate with `filesquid-c2pa`). Extra search/graph stores would be a second stack.

## Assumptions

- HTTP recorder implementation (how bodies are captured) remains a later ADR; this ADR does not pick CDP vs proxy.
- Publisher choice for [FE-042](../product-vision.md#paid-c2pa-publication-watermarking-and-ai-disclosure) remains a supplier decision.

## Alternatives Considered

- **Neo4j / other graph DB for provenance:** Rejected.
- **Python c2pa bindings:** Extra language. Rejected.
- **Only C2PA, no PROV-O:** Would miss unsigned local history. Rejected.

## Consequences / Implications

Easier: files not a graph cluster. Harder: context pinning and C2PA format matrix still to specify. Recorder and watermark vendor are not decided here.

## Related Decisions / Notes

- [ADR 010](./010-ocfl-1-1-dual-profile.md), [ADR 011](./011-native-media-adapters-and-ffmpeg-license.md)
- [PROV-O](https://www.w3.org/TR/prov-o/), [JSON-LD](https://www.w3.org/TR/json-ld11/), [C2PA](https://spec.c2pa.org/specifications/specifications/2.4/specs/C2PA_Specification.html)
