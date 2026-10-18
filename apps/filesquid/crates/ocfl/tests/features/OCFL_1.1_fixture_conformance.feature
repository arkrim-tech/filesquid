# F-051: Reusable filesquid-ocfl engine
# OCFL-S-009, OCFL-S-010, OCFL-S-011: OCFL 1.1 community fixture conformance
# @fixtures: reads the pinned 1.1 tree committed under tests/fixtures/ocfl/1.1 (always present).
# The write scenario's steps arrive with IO-0.10; until then cucumber reports it as skipped.
@fixtures
Feature: OCFL 1.1 community fixture conformance
  As a preservation programme using filesquid-ocfl
  I want the engine tested against the community OCFL 1.1 fixtures
  So that its validation, reads and writes comply with the OCFL 1.1 specification

  Scenario: Valid OCFL 1.1 fixture objects pass validation
    Given the pinned OCFL 1.1 community fixtures
    When I validate every object in good-objects
    Then every object should validate with zero errors

  Scenario: Warning OCFL 1.1 fixture objects report their warning codes
    Given the pinned OCFL 1.1 community fixtures
    When I validate every object in warn-objects
    Then every object should validate with zero errors
    And every object should report each warning code in its directory prefix

  Scenario: Invalid OCFL 1.1 fixture objects are rejected
    Given the pinned OCFL 1.1 community fixtures
    When I validate every object in bad-objects
    Then every object should be rejected as invalid
    And each error code in a directory prefix that was not reported should be listed in the Walkthrough

  Scenario: Valid OCFL 1.1 fixture objects are readable at every version
    Given the pinned OCFL 1.1 community fixtures
    When I resolve every version of every object in good-objects
    Then every streamed logical file should match its inventory digest

  Scenario: Objects written from OCFL 1.1 content fixtures pass independent validation
    Given the pinned OCFL 1.1 community fixtures
    When I write one object version per version directory of every content fixture
    Then every written object should validate with zero errors in filesquid-ocfl
    And every written object should validate with zero errors in an independent community validator
    And the logical state of every written version should equal its source content

  Scenario: Community fixtures are pinned test data only
    When I inspect the OCFL fixture pin
    Then the fixtures should be pinned to one OCFL/fixtures commit
    And the fixtures should not be packaged in or loaded by the filesquid-ocfl library or CLI

  # Source
  # - resources/prd.md F-051
  # - resources/srs.md SRS-F-051
  # - apps/filesquid/crates/ocfl/resources/srs.md OCFL-S-009, OCFL-S-010, OCFL-S-011
  # - resources/architecture/023-filesquid-ocfl-engine-and-parquet.md
  # - https://github.com/OCFL/fixtures (1.1/ at commit d323fc207126becec066998b43408d998bf0fb1b)
