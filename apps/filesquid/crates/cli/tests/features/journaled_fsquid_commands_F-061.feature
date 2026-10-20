# F-061: Activity journal and live health monitoring
# SRS-F-061: Activity journal and live health monitoring
# API-S-031: Journal query and export (the `fsquid` CLI journals through the same path; ADR 036 Decision 8)
# Bound in tests/support/journal_steps.rs (Task CLI-0.2).
Feature: Every fsquid command that changes something or fails is journaled
  As a library owner
  I want my command-line work recorded in the same journal as the app's
  So that no activity goes unrecorded, whichever tool did it

  Background:
    Given a fresh FileSquid data directory

  # Happy path
  Scenario: A command that commits is journaled once with opaque IDs only
    When I set a title on a new object with fsquid
    Then the journal should hold one "cli.metadata.set" entry that "succeeded"
    And that entry should name the object and its version but no path or value

  # Edge case
  Scenario: A command that changes nothing is not journaled
    When I set a title on a new object with fsquid
    And I set the same title again with fsquid
    And I read the title with fsquid
    Then the journal should hold 1 entry

  # Edge case
  Scenario: A successful export writes a file outside the object and is journaled
    When I set a title on a new object with fsquid
    And I export the object's metadata as CSV with fsquid
    Then the journal should hold 2 entries
    And the last entry should be a "cli.metadata.export" entry that "succeeded"

  # Edge case
  Scenario: fsquid writes beside a running desktop host
    Given another connection holds the journal open
    When I set a title on a new object with fsquid
    Then that connection should see the "cli.metadata.set" entry

  # Error case
  Scenario: A command line that names no module command is journaled as a failed route
    When I run the fsquid binary with "validate dcterms:title"
    Then the journal should hold one "cli.route" entry that "failed" with "module_required"

  # Error case
  Scenario: A journal that cannot be opened refuses the command
    Given a data directory that cannot hold a journal
    When I set a title on a new object with fsquid
    Then fsquid should refuse with one journal_unavailable line naming "cli.metadata.set"
    And no object should have been written
