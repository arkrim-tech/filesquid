# Task MD-0.7 Walkthrough: Namespace registry and extension statements

Global task: [FS-1.3](../../../../../../resources/implementation-plan.md#task-fs-1.3). Plan row: [MD-0.7](../plan.md). Requirements: [MD-S-012](../srs.md), [MD-S-002](../srs.md), [MD-S-010](../srs.md).

## MD-S-012: Namespace registry

An owner, a distribution or a module registers an extension namespace (prefix, IRI, optional term list), and the owner may declare a custom namespace of free keys. Registrations never redefine `dcterms:`, `dc:`, `schema:` or `exif:`, a prefix keeps exactly one IRI, a term list rejects unknown terms, an unregistered prefix is rejected by name, extension statements are their own `metadata.md` map, round-trip through CSV, JSON and XMP and are never mapped onto `dcterms:`, and registrations persist in canonical OCFL state.

## MD-S-002: `/terms/` register

The register is unchanged for the built-in namespaces (unknown `/terms/` names and file `accrual*` terms rejected, `dc:` mapped onto `dcterms:`); any other owner-authored statement must use a registered extension namespace.

## MD-S-010: JSON sidecar import/export

JSON import applies the `/terms/` register and the namespace registry checks; an unregistered prefix is named in the error.

## What changed

- `namespace` (new, pure, `wasm32`-safe): `NamespaceRegistry`, `Registration::extension` / `Registration::custom`, `NamespaceKind`, `Registrant`, `RegistryError`, `NAMESPACES_MD`, `REGISTRY_SCHEMA`, `MAX_REGISTRATIONS` (1,024), `MAX_TERMS` (10,000), `valid_term`, `is_extension_prefix`.
  - Refused: the built-in prefixes and their IRIs (`http://purl.org/dc/terms/`, `http://purl.org/dc/elements/1.1/`, `https://schema.org/`, `http://schema.org/`, `http://ns.adobe.com/exif/1.0/`) under any prefix, the record-format keys (`schemaorg`, `import`, `namespaces`, the file-record header keys), a prefix already registered with another IRI, an IRI registered under a second prefix, a change between extension and custom, a re-registration that drops terms, a custom namespace with a term list or not declared by the owner.
  - Syntax: prefix 1 to 64 bytes of `[a-z][A-Za-z0-9_-]*`, not starting with `xml`; IRI with a scheme, at most 2,048 bytes, no space, control character, `"`, `<` or `>`; term 1 to 255 bytes of `[A-Za-z_][A-Za-z0-9_.-]*` (a YAML key and an XML element name). The same IRI may add terms.
  - Persisted as the record `filesquid.namespaces.v1` (one JSON registration per line), parsed strictly and checked again on read; `check_extends` is the append-only rule of a save.
- `MetadataNamespace::Extension(prefix)` and `MetadataNamespace::prefix()`; `MetadataError::UnregisteredNamespace(prefix)` and `MetadataError::UnknownExtensionTerm { prefix, term }`.
- `record`: `Record::set_registered(&registry, ..)`; `Record::set` keeps its signature and, with no registry, rejects an extension prefix by name; `parse_curie_with(curie, &registry)`; `parse_curie` now names an unknown prefix (`UnregisteredNamespace`) instead of `UnknownDctermsTerm`.
- `markdown`: one YAML map per extension prefix after the built-in maps, in first-appearance order; reading keeps every map whose key could be a prefix, and only indented lines belong to a map.
- `csv`: `apply_csv_with`, `parse_csv_header_with`; `CsvError::UnregisteredNamespace(prefix)` replaces `UnknownPrefix`, `CsvError::Registry` for a term outside a list; `dc:` headers now map onto `dcterms:` as MD-S-002 asks.
- `json`: `apply_json_with`; statements resolve through the registry.
- `xmp`: `to_xmp_with` binds each extension prefix to its registered IRI (an unregistered prefix is an error naming it); `from_xmp_with` reads a namespace by its IRI under any packet prefix and never as `dcterms:`. `to_xmp` and `from_xmp` are unchanged.
- `store`: `OcflStore::read_registry`, `OcflStore::commit_registry_with` (one version per change; drops refused with `StoreError::Registry`); `LocalObject::load_registry` and `save_registry` at `namespaces.md`.
- CLI: `namespace register <prefix> <iri> [--terms a,b] [--custom]`, `namespace list`, `--registry <dir>`; `set`, `import` and `export` use the registry; `validate [prefix:term]` checks a term or, without one, every statement of the record and names each unregistered prefix.
- Docs: crate README (Namespace registry section), module SRS MD-S-005 and MD-S-012, root PRD F-052 and SRS-F-052 command lists, ADR 036 (metadata command list, dated revision).

## Notes for API-F1.4 (platform engineer)

- `MetadataNamespace` is no longer `Copy` (it carries the extension prefix) and has a new variant; `Statement::prefix()` returns `&str`. Clone where a namespace was copied, and match `Extension(_)`.
- The API needs one registry per repository: read it with `OcflStore::read_registry` and commit it with `commit_registry_with` at a `FileRef` whose record path names it (the object and path are the API's choice, for example `namespaces.md` in the library's snapshot object), and pass it to `Record::set_registered`, `parse_curie_with`, `apply_csv_with`, `apply_json_with`, `to_xmp_with` and `from_xmp_with`.
- A custom namespace needs an IRI; the API can mint `urn:uuid:<uuid>` when the owner gives none.

## Choices that depend on open owner questions

- The prefix `schemaorg` is reserved because a file record writes its Schema.org statements under `schemaorg:` (open question (a) of MD-0.8). If the owner settles (a) differently, the reserved list changes with it.
- No statement disclosure policy exists in the crate yet (F-046 owns retained-field selection, scrubbing and the preview). Extension statements carry their prefix exactly like `exif:`, `dcterms:` and `schema:`, so that policy addresses them the same way.

## SRS traceability

| Requirement | Tests |
|---|---|
| MD-S-012 Namespace registry: register an extension namespace (prefix, IRI, optional term list) by owner, distribution or module | `registration::a_distribution_registers_an_extension_with_a_term_list`, `a_module_registers_an_extension_without_a_term_list`, BDD "A distribution registers an extension namespace and authors a statement in it" |
| MD-S-012 Namespace registry: owner-declared custom namespace of free keys | `an_owner_declares_a_custom_namespace_of_free_keys`, `a_custom_key_must_be_a_valid_term_name`, BDD "An owner declares a custom namespace of free keys" |
| MD-S-012 Namespace registry: no redefinition of `dcterms:`, `dc:`, `schema:`, `exif:` | `the_built_in_namespaces_cannot_be_redefined`, `a_built_in_iri_cannot_be_registered_under_another_prefix`, `the_record_format_keys_cannot_be_prefixes`, BDD outline "A registration cannot redefine a built-in namespace", CLI `namespace_register_refuses_a_built_in_prefix_and_another_iri` |
| MD-S-012 Namespace registry: unique prefix, one IRI | `re_registering_a_prefix_with_another_iri_is_rejected`, `an_iri_belongs_to_one_prefix`, `an_identical_re_registration_changes_nothing`, `the_same_iri_may_add_terms_but_never_remove_them`, BDD "A prefix keeps its one IRI" |
| MD-S-012 Namespace registry: a term list rejects unknown terms | `a_term_list_rejects_unknown_terms`, `a_csv_header_outside_a_term_list_is_rejected`, `json_import_rejects_a_term_outside_the_list`, BDD "A term list rejects unknown terms" |
| MD-S-012 Namespace registry: unregistered prefix rejected by name | `an_unregistered_prefix_is_rejected_by_name`, `a_curie_resolves_a_registered_prefix_and_names_an_unregistered_one`, `a_csv_header_with_an_unregistered_prefix_is_rejected_by_name`, `json_import_applies_the_registry`, `xmp_export_rejects_an_unregistered_prefix_by_name`, `xmp_export_rejects_a_term_that_is_not_an_element_name`, CLI `set_rejects_an_unregistered_prefix_or_an_unlisted_term_by_name`, BDD "A statement in an unregistered namespace is rejected by name" |
| MD-S-012 Namespace registry: own YAML map in `metadata.md` | `each_extension_prefix_is_its_own_yaml_map`, `a_map_of_an_unregistered_prefix_is_kept_on_read`, `only_indented_lines_belong_to_a_map`, CLI `a_registered_extension_statement_is_set_in_its_own_map` |
| MD-S-012 Namespace registry: round-trip through `metadata.md`, CSV, JSON, XMP | `extension_statements_round_trip_unchanged`, `..._through_csv`, `..._through_json`, `..._through_xmp`, `xmp_import_reads_a_namespace_by_its_iri_under_any_packet_prefix`, CLI `extension_statements_round_trip_through_cli_csv_and_xmp`, BDD "Extension statements round-trip through metadata.md, CSV, JSON and XMP" |
| MD-S-012 Namespace registry: never mapped onto `dcterms:` | `an_extension_statement_is_never_a_dcterms_statement`, `xmp_import_reads_a_namespace_by_its_iri_under_any_packet_prefix`, BDD "no dcterms statement should be written" |
| MD-S-012 Namespace registry: registrations persist through the canonical store | `unit_namespace_store::*`, `persistence::*`, CLI `a_separate_registry_object_is_named_with_registry`, BDD "Registrations persist in canonical OCFL state" |
| MD-S-012 Namespace registry: CLI `validate` reports unregistered prefixes | CLI `validate_reports_unregistered_prefixes`, BDD "The CLI validate command reports an unregistered prefix" |
| MD-S-002 `/terms/` register | `the_registry_keeps_the_dcterms_register_and_the_dc_mapping`, existing `unit_register`, `unit_csv`, BDD register scenarios |
| MD-S-010 JSON sidecar import/export: registry checks on import | `json_import_applies_the_registry`, `json_import_rejects_a_term_outside_the_list`, BDD "JSON sidecar import rejects an unregistered namespace" |
| MD-S-009 `wasm32` build | `cargo check --target wasm32-unknown-unknown` with default features off and with `ocfl` (below), BDD `@wasm32` scenarios |

## TDD: failing run before implementation

The unit and store tests did not compile against the old crate; the CLI tests ran and failed; the 12 new BDD scenarios had no steps (skipped):

```text
      1 error: could not compile `filesquid-metadata` (test "unit_namespace") due to 26 previous errors
      1 error: could not compile `filesquid-metadata` (test "unit_namespace_store") due to 13 previous errors
      1 error[E0432]: unresolved import `filesquid_metadata::json::apply_json_with`
      2 error[E0432]: unresolved import `filesquid_metadata::namespace`
      1 error[E0432]: unresolved import `filesquid_metadata::record::parse_curie_with`
      1 error[E0432]: unresolved imports `filesquid_metadata::csv::apply_csv_with`, `filesquid_metadata::csv::parse_csv_header_with`
      1 error[E0432]: unresolved imports `filesquid_metadata::xmp::from_xmp_with`, `filesquid_metadata::xmp::to_xmp_with`
      7 error[E0599]: no method named `commit_registry_with` found for struct `OcflStore<'a, B, R, S, C, L>` in the current scope
      4 error[E0599]: no method named `read_registry` found for struct `OcflStore<'a, B, R, S, C, L>` in the current scope
      7 error[E0599]: no method named `set_registered` found for struct `filesquid_metadata::record::Record` in the current scope
      5 error[E0599]: no variant named `UnknownExtensionTerm` found for enum `filesquid_metadata::MetadataError`
      1 error[E0599]: no variant, associated function, or constant named `Extension` found for enum `filesquid_metadata::MetadataNamespace` in the current scope
      1 error[E0599]: no variant, associated function, or constant named `Registry` found for enum `StoreError` in the current scope
      1 error[E0599]: no variant, associated function, or constant named `Registry` found for enum `filesquid_metadata::csv::CsvError` in the current scope
      5 error[E0599]: no variant, associated function, or constant named `UnregisteredNamespace` found for enum `filesquid_metadata::MetadataError` in the current scope
      2 error[E0599]: no variant, associated function, or constant named `UnregisteredNamespace` found for enum `filesquid_metadata::csv::CsvError` in the current scope
test a_registered_extension_statement_is_set_in_its_own_map ... FAILED
test a_separate_registry_object_is_named_with_registry ... FAILED
test extension_statements_round_trip_through_cli_csv_and_xmp ... FAILED
test namespace_register_refuses_a_built_in_prefix_and_another_iri ... FAILED
test set_rejects_an_unregistered_prefix_or_an_unlisted_term_by_name ... FAILED
test validate_reports_unregistered_prefixes ... FAILED
test result: FAILED. 22 passed; 6 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.12s
55 scenarios (42 passed, 13 skipped)
141 steps (128 passed, 13 skipped)
```

## Mocked Environment Variables

None.

## Raw test output

`cargo test -p filesquid-metadata --all-features --locked --test unit_namespace --test unit_namespace_store`:

```text
    Finished `test` profile [unoptimized] target(s) in 0.16s
     Running tests/unit/namespace.rs (<target>/debug/deps/unit_namespace-514c5d7779995504)
running 39 tests
test csv::a_csv_header_outside_a_term_list_is_rejected ... ok
test csv::a_csv_header_with_an_unregistered_prefix_is_rejected_by_name ... ok
test csv::extension_statements_round_trip_through_csv ... ok
test json::json_import_rejects_a_term_outside_the_list ... ok
test json::extension_statements_round_trip_through_json ... ok
test markdown::a_map_of_an_unregistered_prefix_is_kept_on_read ... ok
test json::json_import_applies_the_registry ... ok
test markdown::each_extension_prefix_is_its_own_yaml_map ... ok
test markdown::only_indented_lines_belong_to_a_map ... ok
test markdown::extension_statements_round_trip_unchanged ... ok
test persistence::a_malformed_registry_record_is_refused ... ok
test persistence::a_registry_save_may_only_add ... ok
test persistence::a_stored_registration_that_breaks_a_rule_is_refused_on_read ... ok
test registration::a_built_in_iri_cannot_be_registered_under_another_prefix ... ok
test persistence::the_registry_round_trips_through_its_markdown_record ... ok
test registration::a_custom_namespace_cannot_become_an_extension ... ok
test registration::a_distribution_registers_an_extension_with_a_term_list ... ok
test registration::a_module_registers_an_extension_without_a_term_list ... ok
test registration::an_identical_re_registration_changes_nothing ... ok
test registration::an_iri_belongs_to_one_prefix ... ok
test registration::an_owner_declares_a_custom_namespace_of_free_keys ... ok
test registration::duplicate_terms_in_a_list_are_refused ... ok
test registration::re_registering_a_prefix_with_another_iri_is_rejected ... ok
test registration::prefixes_terms_and_iris_must_be_well_formed ... ok
test registration::the_built_in_namespaces_cannot_be_redefined ... ok
test registration::the_same_iri_may_add_terms_but_never_remove_them ... ok
test statements::a_curie_resolves_a_registered_prefix_and_names_an_unregistered_one ... ok
test statements::a_custom_key_must_be_a_valid_term_name ... ok
test statements::a_term_list_rejects_unknown_terms ... ok
test statements::an_extension_statement_is_never_a_dcterms_statement ... ok
test statements::an_unregistered_prefix_is_rejected_by_name ... ok
test statements::the_registry_keeps_the_dcterms_register_and_the_dc_mapping ... ok
test statements::without_a_registry_an_extension_statement_is_unregistered ... ok
test xmp::xmp_export_rejects_a_term_that_is_not_an_element_name ... ok
test xmp::extension_statements_round_trip_through_xmp ... ok
test xmp::xmp_export_rejects_an_unregistered_prefix_by_name ... ok
test xmp::xmp_import_reads_a_namespace_by_its_iri_under_any_packet_prefix ... ok
test registration::the_record_format_keys_cannot_be_prefixes ... ok
test registration::registrations_and_term_lists_are_bounded ... ok
test result: ok. 39 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
     Running tests/unit/namespace_store.rs (<target>/debug/deps/unit_namespace_store-80bfb797b0c3af3d)
running 5 tests
test an_object_without_a_registry_reads_as_empty ... ok
test a_registration_is_committed_in_one_version_and_read_back ... ok
test a_save_that_drops_a_registration_is_refused_and_publishes_nothing ... ok
test an_unchanged_registry_publishes_nothing ... ok
test a_new_registration_is_a_new_version ... ok
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

`cargo test -p filesquid-metadata --all-features --locked` (every binary, including the BDD run with both `wasm32` builds):

```text
     Running unittests src/lib.rs (<target>/debug/deps/filesquid_metadata-f851fec434ee5da3)
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/features.rs (<target>/debug/deps/features-08e86a57382d7750)
55 scenarios (54 passed, 1 skipped)
166 steps (165 passed, 1 skipped)
     Running tests/unit/cli.rs (<target>/debug/deps/unit_cli-fca5a15484d2aaf5)
test result: ok. 28 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.12s
     Running tests/unit/csv.rs (<target>/debug/deps/unit_csv-5bd695e62e5a615c)
test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/unit/extract.rs (<target>/debug/deps/unit_extract-cb8ec81a5ddb769e)
test result: ok. 13 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/unit/file_record_store.rs (<target>/debug/deps/unit_file_record_store-45934ecbe5501a6e)
test result: ok. 25 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
     Running tests/unit/file_ref.rs (<target>/debug/deps/unit_file_ref-a62b5dba5ec281fe)
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/unit/import_record.rs (<target>/debug/deps/unit_import_record-7a4d4c280bde0e84)
test result: ok. 59 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
     Running tests/unit/isolation.rs (<target>/debug/deps/unit_isolation-2ad164b104aef085)
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/unit/json.rs (<target>/debug/deps/unit_json-5f5532124da20da9)
test result: ok. 19 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/unit/markdown.rs (<target>/debug/deps/unit_markdown-d013e68ff32a4bec)
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/unit/namespace.rs (<target>/debug/deps/unit_namespace-514c5d7779995504)
test result: ok. 39 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
     Running tests/unit/namespace_store.rs (<target>/debug/deps/unit_namespace_store-80bfb797b0c3af3d)
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/unit/register.rs (<target>/debug/deps/unit_register-293af9f1979c216e)
test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/unit/sidecar.rs (<target>/debug/deps/unit_sidecar-5462564e5b35473b)
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/unit/store.rs (<target>/debug/deps/unit_store-eab858085c3416e1)
test result: ok. 28 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.32s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

`cargo clippy -p filesquid-metadata --all-targets --all-features --locked -- -D warnings`: clean. `cargo check -p filesquid-metadata --lib --no-default-features [--features ocfl] --target wasm32-unknown-unknown --locked`: both `Finished`.

The README commands (`namespace register`, `set rico:title`, `metadata validate`, `namespace list`) were run with the built `fsquid` against a scratch object: `committed v2`, `committed v3`, `every statement is in a built-in or registered namespace` (exit 0), and `set lido:title` and `metadata validate lido:title` refused naming `lido`.
