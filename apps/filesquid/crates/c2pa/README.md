# filesquid-c2pa (planned)

The C2PA module of FileSquid. It is a FOSS [C2PA](https://spec.c2pa.org/specifications/specifications/2.4/specs/C2PA_Specification.html) engine, implemented from scratch in Rust. It reads, validates, mints, edits and embeds C2PA manifests, and produces collection scan reports. Any developer can add C2PA to their own application through its API.

| | |
|---|---|
| Cargo package | `filesquid-c2pa` |
| Rust import | `filesquid_c2pa` |
| Path | `apps/filesquid/crates/c2pa/` (member of the `apps/filesquid/` Cargo workspace once the crate exists) |
| Status | Planned. This directory holds docs and the feature file only; the crate is scaffolded at global Task 0.17. |

- C2PA logic (JUMBF, CBOR claims and assertions, bindings, validation, manifest assembly, format handlers) is this module's own code. Cryptographic primitives come from established crates behind a crypto-backend trait; they are never hand-rolled.
- The official `c2pa` crate and `c2patool` are interoperability oracles in CI only, never the implementation or a runtime dependency.
- Trust lists and signers are supplied by the caller. The module operates no certificate authority.
- No default dependency on `filesquid-ocfl`, Axum or Tauri. The module reads and writes through caller-supplied byte sources and sinks. `filesquid-api` connects it to the OCFL module.
- Builds for `wasm32-unknown-unknown`, so the roadmap E2EE browser host stays reachable.
- Provides the `fsquid` commands `read`, `verify`, `sign`, `edit`, `embed`, `report` (the crate ships no binary; `filesquid-cli` builds `fsquid`). PROV-O JSON-LD projection is an optional feature.

The Content Credentials sidebar panel is part of `@filesquid/ui` in `apps/filesquid/ui/`.

## Build and test

Once the crate exists, run Cargo from the workspace root:

```bash
cd apps/filesquid
cargo test -p filesquid-c2pa --locked
```

## References

- [Product vision: modules](../../../../resources/product-vision.md#modules)
- [PRD F-054: C2PA module](../../../../resources/prd.md#f-054-c2pa-module)
- [ADR 026](../../../../resources/architecture/026-filesquid-provenance-engine.md)
- [Module SRS](./resources/srs.md), [module plan](./resources/plan.md) and [component diagram](./resources/component-diagram.html)
