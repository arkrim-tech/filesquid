# SRS-F-043: C2PA records and action disclosure
# ADR 015: PROV-O in versioned JSON-LD plus official c2pa crate
Feature: C2PA and PROV-O record types
  As a host crate
  I want resource-bound import records and PROV-O sidecars
  So that provenance starts at import without mutating originals

  Scenario: An import record is bound to resource identity and SHA-256
    Given a file resource "res-1" with digest "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
    When I initialize a C2PA import record at "2026-09-23T12:00:00Z"
    Then the import record resource should be "res-1"
    And the import digest should be "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
    And the import digest algorithm should be "sha-256"
    And the import time should be "2026-09-23T12:00:00Z"
    And the validation status should be "unsigned"

  Scenario: A prepared unsigned record is not a valid signed manifest
    Given a file resource "res-1" with digest "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
    When I initialize a C2PA import record at "2026-09-23T12:00:00Z"
    Then the unsigned record should not be a valid signed manifest

  Scenario: Signed, invalid, and unavailable statuses are distinct
    When I set validation status to "signed"
    Then the validation status should be "signed"
    And the record should be a valid signed manifest
    When I set validation status to "invalid"
    Then the validation status should be "invalid"
    When I set validation status to "unavailable"
    Then the validation status should be "unavailable"

  Scenario: Catalog-only action does not claim a pixel change
    When I append a catalog-only C2PA action
    Then the action should not claim a pixel change

  Scenario: Ephemeral cache is not permanent PROV-O history
    When I record an ephemeral cache activity at "2026-09-23T12:00:00Z"
    Then the activity should not be permanent PROV-O history

  Scenario: Importing an existing file does not invent capture
    Given a file resource "res-1" with digest "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
    When I initialize a C2PA import record at "2026-09-23T12:00:00Z"
    Then the import should not invent an original capture event

  Scenario: A signed manifest cannot be mutated in place
    When I set validation status to "signed"
    And I mutate the signed manifest in place
    Then mutating the signed manifest should fail

  Scenario: A PROV-O sidecar records UTC time
    When I construct a PROV-O sidecar at "2026-09-23T12:00:00Z"
    Then the sidecar time should be "2026-09-23T12:00:00Z"
    And the sidecar profile should be JSON-LD

  # Source
  # - resources/srs.md SRS-F-043: C2PA records and action disclosure
  # - resources/prd.md F-043: C2PA records and action disclosure
  # - resources/architecture/015-prov-o-jsonld-and-c2pa-sdk.md
