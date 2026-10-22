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

  # MD-S-008: per-edit OCFL versioning (owner decision 2026-10-18, ADR 027)
  Scenario: Each metadata edit is a new OCFL version
    Given an OCFL object with a logical file and dcterms:title Harbour
    When I set dcterms:title Dawn through the ocfl store
    Then the object should have a new version whose metadata.md holds dcterms title Dawn
    And the earlier versions should be unchanged
    And the object should pass the OCFL validator

  Scenario: An unchanged record publishes no version
    Given an OCFL object with a logical file and dcterms:title Harbour
    When I set dcterms:title Harbour through the ocfl store
    Then no version should be published

  # MD-S-014: commit options (FileSquid's user, message and PROV-O record)
  Scenario: A caller's commit options go into the same OCFL version
    When I commit a metadata edit with a version user, an operation message and a version-bound record
    Then the edit should publish one version carrying that user, that message and the record

  # MD-S-008, MD-S-013: the per-file record with its import section (owner decision 2026-10-19, ADR 038)
  Scenario: A file's one record lives beside its stored original in the same version
    When I import a file named "Holiday 2023.JPG" into an OCFL object
    Then the version should hold the stored original and its per-file metadata record
    And the object should hold no metadata.md
    And the imported object should pass the OCFL validator

  Scenario: The record opens with the original name, display name, extension, format and media type
    When I import a file named "Holiday 2023.JPG" into an OCFL object
    Then the record should open with the schema identifier and the original name Holiday 2023.JPG
    And the display name should be the original name
    And the extension, format and media type should be recorded

  Scenario: A re-import appends an entry in a new OCFL version
    When I import a file named "Holiday 2023.JPG" into an OCFL object
    And I import the same file again
    Then the object should have a new version with two import entries
    And the first version should be unchanged

  Scenario: A save that changes an import entry is refused
    When I import a file named "Holiday 2023.JPG" into an OCFL object
    And I save the record with a changed import entry
    Then the save should be refused for changing an import entry
    And the object's versions should be unchanged

  Scenario: A save that removes an import entry is refused
    When I import a file named "Holiday 2023.JPG" into an OCFL object
    And I import the same file again
    And I save the record with only the first import entry
    Then the save should be refused for removing an import entry

  Scenario: A save that changes the original file name is refused
    When I import a file named "Holiday 2023.JPG" into an OCFL object
    And I save the record with another original name
    Then the save should be refused for changing the original name
    And the object's versions should be unchanged

  Scenario: A rename changes only the display name
    When I import a file named "Holiday 2023.JPG" into an OCFL object
    And I rename the file to "Harbour.jpg"
    Then the display name should be Harbour.jpg and the original name Holiday 2023.JPG
    And the import entries should be unchanged

  Scenario: A statements edit keeps the header and the import section
    When I import a file named "Holiday 2023.JPG" into an OCFL object
    And I set dcterms:title Harbour on the file's record
    Then the record should hold dcterms title Harbour and still hold its original name and import entry

  Scenario: An import commit checks the version its entry cites
    When I import a file named "Holiday 2023.JPG" into an OCFL object
    And another writer commits a version before I import the same file again citing v2
    Then the re-import should be refused as a retryable conflict
    And the object should hold only the other writer's new version
    And a retry citing v3 should append the entry in v3

  Scenario: A statements edit still rebases over another writer's version
    When I import a file named "Holiday 2023.JPG" into an OCFL object
    And another writer commits a version while I set dcterms:title Harbour on the file's record
    Then the edit should be published as v3 on top of the other writer's version

  Scenario: The original file name is withheld from an outgoing copy by default
    When I import a file named "Holiday 2023.JPG" into an OCFL object
    And I make the outgoing copy of its record with every identity class
    Then its original name, display name and every source name should be withheld by policy
    And the store should refuse to save that copy

  Scenario: Use original filename keeps the names in the outgoing copy
    When I import a file named "Holiday 2023.JPG" into an OCFL object
    And I make the outgoing copy of its record with every identity class except the original name
    Then its original name, display name and source name should be Holiday 2023.JPG

  Scenario: A source size that differs from the verified size fails the import
    When I import a file whose reported size differs from the verified size
    Then the import should be rejected for the size mismatch
    And nothing should be published

  Scenario: Names and values that are not UTF-8 round-trip as base64
    When I import a file whose name and extended attribute are not UTF-8
    Then the record should store them as base64 marked with encoding base64
    And they should read back byte for byte

  Scenario: Every missing fact is listed as unavailable with a reason
    When I import a file through an upload that provides only a name, size, time and media type
    Then every other source, volume and attribute fact should be unavailable with the reason not provided by upload

  Scenario: Attribute inspection stops at 256 names and 1 MiB and marks what it cut
    When I inspect 300 extended attributes of 8 KiB each
    Then at most 256 names should be kept and the entry marked truncated with 300 names enumerated
    And at most 1 MiB of value bytes should be recorded as read
    And every value cut by the budget should have an incomplete digest
    And a re-import should get its own budget of 256 names and 1 MiB

  Scenario: A value over 64 KiB is kept as size and SHA-256 only
    When I inspect an extended attribute of 70000 bytes
    Then the record should hold its size and SHA-256 and not the value

  Scenario: The read projection omits paths, host and account
    When I import a file with a full set of source facts and read its projection
    Then the projection should show the original name, relative path, volume name and OCFL object
    And it should not show the absolute path, the import root, the host name or any account

  Scenario: The outgoing policy scrubs the identity fields
    When I scrub every identity class of a full import entry
    Then no path, account, host name, numeric ID, origin URL, quarantine agent or Finder comment should remain
    And each removed fact should be listed as withheld by policy

  Scenario: The import record builds for the browser without filesystem access
    When I inspect the import record modules
    Then they should use no filesystem, path or environment access

  Scenario: A head without its version inventory stays refused
    Given an OCFL object whose head has no version inventory
    When I set dcterms:title Dawn through the ocfl store
    Then the edit should be refused for the missing version inventory
    And no version should be published
    And the missing version inventory should not be repaired

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

  # MD-S-012: namespace registry and extension statements
  Scenario: A distribution registers an extension namespace and authors a statement in it
    Given the namespace rico is registered with a term list
    When I author rico:title "Fonds 12" on a file
    Then metadata.md should hold rico title "Fonds 12" in its own map
    And no dcterms statement should be written

  Scenario: An owner declares a custom namespace of free keys
    Given the owner declares the custom namespace mine
    When I author mine:shelf_mark "B-7" on a file
    Then metadata.md should hold mine shelf_mark "B-7" in its own map

  Scenario Outline: A registration cannot redefine a built-in namespace
    When I register the namespace <prefix>
    Then the registration should be refused naming <prefix>

    Examples:
      | prefix  |
      | dcterms |
      | dc      |
      | schema  |
      | exif    |

  Scenario: A prefix keeps its one IRI
    Given the namespace rico is registered with a term list
    When I register rico again with another IRI
    Then the registration should be refused naming rico
    And rico should keep its first IRI

  Scenario: A term list rejects unknown terms
    Given the namespace rico is registered with a term list
    When I author rico:colour "red" on a file
    Then the write should be rejected naming colour

  Scenario: A statement in an unregistered namespace is rejected by name
    Given the namespace rico is registered with a term list
    When I author lido:title "x" on a file
    Then the write should be rejected naming lido

  Scenario: Extension statements round-trip through metadata.md, CSV, JSON and XMP
    Given the namespace rico is registered with a term list
    When I author rico:title "Fonds 12" on a file
    Then the statement should round-trip unchanged through metadata.md, CSV, JSON and XMP

  Scenario: Registrations persist in canonical OCFL state
    Given the namespace rico is registered with a term list
    When I commit the registry to an OCFL object
    Then a store opened afresh over that object should recover the registration
    And the registry object should pass the OCFL validator

  Scenario: The CLI validate command reports an unregistered prefix
    Given an OCFL object whose registry holds rico
    When I validate lido:title with the CLI
    Then validate should fail
    When I validate rico:title with the CLI
    Then validate should succeed

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

  # MD-S-007: XMP packets map onto dcterms
  Scenario: Extraction maps an embedded XMP packet onto dcterms
    When I extract metadata from bytes holding an XMP packet with dc:title Harbour
    Then the extracted dcterms:title should be Harbour

  Scenario: Extraction takes a reader and a file reference, independent of any store
    When I inspect the filesquid-metadata extraction API
    Then extraction should take bytes or a reader plus a FileRef and no path

  Scenario: Sidecar store keeps a record beside a plain file
    Given a plain file in a directory
    When I save a record through the sidecar store
    Then the record should be loadable from the sidecar beside the file

  Scenario: CLI and path helpers are native-only
    When I inspect the filesquid-metadata native-only modules
    Then the CLI, the sidecar store and the OCFL path helpers should be compiled out for wasm32

  Scenario: Metadata panel plugs into a file browser sidebar
    Given the filesquid-ocfl file browser with a selected file
    When the host registers the filesquid-metadata Metadata panel
    Then the sidebar should show populated exif, dcterms, and schema rows with prefixes
    And the Metadata panel package should not depend on the filesquid-ocfl UI package

  # Source
  # - resources/prd.md F-052
  # - resources/srs.md SRS-F-052
  # - apps/filesquid/crates/metadata/resources/srs.md MD-S-007 extraction, MD-S-008 canonical metadata.md, MD-S-010 JSON sidecar import/export, MD-S-013 import record, MD-S-014 commit options
  # - resources/architecture/027-filesquid-metadata-pluggable-stores.md (per-edit OCFL versioning)
  # - resources/architecture/022-dublin-core-descriptive-metadata.md (Sidecar JSON)
  # - resources/architecture/024-filesquid-metadata-engine.md
  # - resources/architecture/025-client-side-filesquid-engines.md
  # - resources/architecture/028-filesquid-ui-packages-and-panel-composition.md
