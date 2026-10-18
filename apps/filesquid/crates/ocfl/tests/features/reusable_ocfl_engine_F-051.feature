# F-051: OCFL module
# SRS-F-051: OCFL module
Feature: OCFL module (filesquid-ocfl)
  As an institution or another program
  I want to install filesquid-ocfl as its own FOSS library or tool
  So that I can use OCFL 1.1 without installing the FileSquid app

  Scenario: Core API is generic OCFL 1.1
    When I inspect the filesquid-ocfl public API
    Then FileSquid catalog, recipe, C2PA, and collection-snapshot types should be absent

  Scenario: Default dependencies exclude analytical engines
    When I inspect filesquid-ocfl library dependencies
    Then Arrow, Parquet, DuckDB, and Iceberg should be absent from default features

  Scenario: Stand-alone FOSS product
    When I inspect filesquid-ocfl product identity
    Then it should remain installable without the FileSquid app
    And it should build and publish without other FileSquid crates
    And community OCFL programs should not be the engine

  Scenario: FileSquid hosts link the engine library
    When I inspect the desktop crate
    Then desktop Cargo should depend on filesquid-ocfl
    And FileSquid should not shell out to fsquid for canonical writes

  Scenario: Storage goes through one async backend interface
    When I inspect the filesquid-ocfl public API
    Then a storage backend trait with local filesystem and in-memory backends should be available
    And the engine core should not call std::fs directly

  Scenario: Committed logical state is enumerated without a database or catalog types
    When I inspect the filesquid-ocfl public API
    Then a committed logical-state enumerator should be available
    And it should need no SQLite, PostgreSQL or FileSquid catalog types

  Scenario: Engine builds for the browser
    When I build filesquid-ocfl for wasm32-unknown-unknown with default features off
    Then the build should succeed

  Scenario: The ciphertext-only hosted service never links the engine
    When I inspect the hosted backup and sync service crate
    Then its Cargo manifest should not depend on filesquid-ocfl

  Scenario: Drive-style browser shows storage facts only
    Given an OCFL object with two versions
    When I open it in the filesquid-ocfl file browser
    Then I should see folders and files from logical paths with size, type hint, modified time, and version
    And the Details panel should show digests, version agent, and message
    And no embedded or descriptive metadata should be extracted by filesquid-ocfl

  Scenario: Browser exposes sidebar panel slots
    When I inspect the filesquid-ocfl UI package dependencies
    Then the browser should expose named sidebar panel slots
    And it should not depend on the metadata or C2PA UI packages

  # Source
  # - resources/prd.md F-051
  # - resources/srs.md SRS-F-051
  # - resources/architecture/023-filesquid-ocfl-engine-and-parquet.md
  # - resources/architecture/025-client-side-filesquid-engines.md
  # - resources/architecture/029-filesquid-unified-product-and-distributions.md
  # - resources/architecture/032-hosting-and-privacy-model.md
  # - resources/architecture/028-filesquid-ui-packages-and-panel-composition.md
