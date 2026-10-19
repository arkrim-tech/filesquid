# OCFL-S-024: Version-bound logical files. A commit may carry a function that returns logical files from the reserved version name and the created time; it runs after the vN reservation and before any vN byte, again after a rebase, its output is digested and written like any staged file, and nothing is published when it fails. VersionMeta user and message pass through unchanged.
# OCFL-S-016: Concurrent commits (a rebased commit binds its files to the new version)
# F-067: PROV-O history of every version and change
# F-051: OCFL module
Feature: Version-bound logical files
  As a caller that records each version inside the version itself
  I want a commit to carry files named after the version the engine reserves
  So that a record names its own version and created time, even after a rebase

  Background:
    Given an object committed at "v1" with "a.txt", "b.txt" and "c.txt" through the change model

  # Happy path
  Scenario: A commit carries a record named after its reserved version
    Given writer "writer-a" staged new bytes for "a.txt"
    When writer "writer-a" commits the staged changes with a version-bound record
    Then the object head should be "v2" through the change model
    And "v2" should hold the record "record/v2.txt" naming "v2" and its created time

  Scenario: A record-only commit adds one version holding only the record
    When writer "writer-a" commits only a version-bound record
    Then the object head should be "v2" through the change model
    And "v2" should hold the record "record/v2.txt" naming "v2" and its created time

  # Edge case
  Scenario: A rebased commit binds its record to the new version
    Given writer "writer-a" staged new bytes for "a.txt"
    And writer "writer-b" committed "v2" with new bytes for "b.txt"
    When writer "writer-a" commits the staged changes with a version-bound record
    Then the object head should be "v3" through the change model
    And "v3" should hold the record "record/v3.txt" naming "v3" and its created time

  # Error case
  Scenario: A failing record function publishes nothing
    Given writer "writer-a" staged new bytes for "a.txt"
    When writer "writer-a" commits the staged changes with a failing record function
    Then the commit should be refused for its version-bound files
    And the object head should be "v1" through the change model

  # Source
  # - apps/filesquid/crates/ocfl/resources/srs.md OCFL-S-024, OCFL-S-016
  # - resources/prd.md F-067, F-051
  # - resources/architecture/040-prov-o-history.md Decision 3
