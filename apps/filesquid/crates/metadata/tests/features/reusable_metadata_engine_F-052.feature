# F-052: Reusable filesquid-metadata engine
# SRS-F-052: Reusable filesquid-metadata engine
Feature: Reusable filesquid-metadata engine
  As an institution or another program
  I want to install filesquid-metadata as its own FOSS metadata CRUD engine
  So that I can extract metadata from OCFL files and create or enhance Dublin Core records without the FileSquid app

  Scenario: Core API is namespaced metadata
    When I inspect the filesquid-metadata public API
    Then dcterms register and statement operations should be available
    And FileSquid catalog, recipe, C2PA, and collection-snapshot types should be absent

  Scenario: Unknown dcterms local names are rejected
    When I author dcterms:name on a file
    Then the write should be rejected

  Scenario: Accrual terms are rejected on a file
    When I author dcterms:accrualMethod on a file
    Then the write should be rejected

  Scenario: Canonical store is metadata.md
    When I set dcterms:title Harbour on an object
    Then metadata.md YAML front matter should contain dcterms title Harbour
    And catalogs should be rebuildable from that file

  Scenario: Persistence is pluggable and filesquid-ocfl is optional
    When I inspect filesquid-metadata library dependencies
    Then filesquid-ocfl should be an optional dependency outside the default features
    And axum, tauri, libvips, and filesquid-core should be absent

  Scenario: CLI is the stand-alone UI
    When I ask the metadata command runner for help
    Then I should see extract, get, set, export, import, and validate

  Scenario: CSV headers are prefix:term
    When I import a CSV with headers id,dcterms:title,dcterms:name
    Then dcterms:name should be rejected

  Scenario: Interchange excludes MODS and SQL
    When I inspect filesquid-metadata interchange
    Then CSV, JSON sidecar, XMP sidecar, and embedded extract should be available
    And MODS and SQL should be absent

  # MD-S-010: JSON sidecar import/export
  Scenario: JSON sidecar binds statements to the object and file digest
    Given an OCFL object with a logical file and dcterms:title Harbour
    When I export the object's JSON sidecar
    Then the sidecar should be one UTF-8 JSON document for that object
    And it should carry the object identifier and the SHA-256 digest of the file
    And each statement should have a namespace, term, and value

  Scenario: JSON sidecar export then import reproduces the statements
    Given an OCFL object with a logical file and dcterms:title Harbour
    When I export the object's JSON sidecar
    And I import that sidecar into an empty object holding the same file
    Then the imported metadata.md should hold the same statements

  Scenario: JSON sidecar import rejects a digest mismatch
    Given an OCFL object with a logical file and dcterms:title Harbour
    When I export the object's JSON sidecar
    And I import that sidecar into an object whose file has different bytes
    Then the import should be rejected for a digest mismatch
    And that object's metadata.md should be unchanged

  Scenario: JSON sidecar import applies the /terms/ register
    When I import a JSON sidecar holding dcterms:name
    Then the JSON import should be rejected

  Scenario: JSON sidecar import rejects an unregistered namespace
    When I import a JSON sidecar holding rico:title
    Then the JSON import should be rejected naming the prefix rico

  Scenario: JSON sidecar does not replace metadata.md
    Given an OCFL object with a logical file and dcterms:title Harbour
    When I export the object's JSON sidecar
    Then metadata.md should remain the canonical store in the object

  Scenario: Stand-alone FOSS product
    When I inspect filesquid-metadata product identity
    Then it should remain installable without the FileSquid app
    And it should build and publish without FileSquid application crates

  @wasm32
  Scenario: Engine builds for the browser
    When I build filesquid-metadata for wasm32-unknown-unknown with default features off
    Then the build should succeed

  @wasm32
  Scenario: Engine builds for the browser with the OCFL store
    When I build filesquid-metadata for wasm32-unknown-unknown with the ocfl feature
    Then the build should succeed

  Scenario: Extraction reads bytes from filesquid-ocfl
    When I extract metadata from a logical file in an OCFL object
    Then the extractor should read bytes supplied by filesquid-ocfl, not a filesystem path

  Scenario: Extraction takes a reader and a file reference, independent of any store
    When I inspect the filesquid-metadata extraction API
    Then extraction should take bytes or a reader plus a FileRef and no path

  Scenario: Sidecar store keeps a record beside a plain file
    Given a plain file in a directory
    When I save a record through the sidecar store
    Then the record should be loadable from the sidecar beside the file

  Scenario: CLI and path helpers are native-only
    When I inspect the filesquid-metadata native-only modules
    Then the CLI and the sidecar store should be compiled out for wasm32

  Scenario: Metadata panel plugs into a file browser sidebar
    Given the filesquid-ocfl file browser with a selected file
    When the host registers the filesquid-metadata Metadata panel
    Then the sidebar should show populated exif, dcterms, and schema rows with prefixes
    And the Metadata panel package should not depend on the filesquid-ocfl UI package

  # Source
  # - resources/prd.md F-052
  # - resources/srs.md SRS-F-052
  # - apps/filesquid/crates/metadata/resources/srs.md MD-S-010 JSON sidecar import/export
  # - resources/architecture/022-dublin-core-descriptive-metadata.md (Sidecar JSON)
  # - resources/architecture/024-filesquid-metadata-engine.md
  # - resources/architecture/025-client-side-filesquid-engines.md
  # - resources/architecture/028-filesquid-ui-packages-and-panel-composition.md
