# SRS-F-043: Import-time C2PA records and action disclosure
Feature: Import-time C2PA records and action disclosure
  As a library owner
  I want import-time c2pa records and action disclosure
  So that the product matches the SRS

  Scenario: Happy path for F-043
    Given the feature is enabled on a supported profile
    When I use it as specified in the PRD
    Then the MUST criteria for that feature should hold

  Scenario: Limitation is visible for F-043
    Given a platform cannot provide a capability
    When I view that capability
    Then I should see an explicit unsupported or waiting state

  Scenario: Failure does not corrupt originals for F-043
    Given an operation fails
    When I inspect the library
    Then originals and committed OCFL snapshots should remain

  Scenario: Files imported before the c2pa module receive honestly dated records for F-043
    Given a library holds files imported before the c2pa module was enabled
    When the c2pa module is enabled and the backfill job runs, is interrupted and resumes
    Then every such file should have exactly one initial record
    And each record should state its own creation time and cite the OCFL version created time and SHA-256 digest
    And no record should claim FileSquid import time

  # Source
  # - srs.md SRS-F-043
  # - prd.md F-043
