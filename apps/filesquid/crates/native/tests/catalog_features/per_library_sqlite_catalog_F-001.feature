# F-001: Independent local library and ownership
# SRS-F-001: Independent local library and ownership (per-library operational catalog)
# F-009: Multiple libraries and filesystem administration
# DE-S-009: Per-library SQLite catalog storage of the desktop host
Feature: Per-library SQLite catalog
  As an owner with more than one library on my computer
  I want each library to keep its own operational catalog beside the canonical files
  So that libraries stay independent, survive restarts and can always be rebuilt

  # Happy path
  Scenario: Each library keeps its own catalog file
    Given an empty catalog directory
    When the owner creates the libraries "Family archive" and "Work"
    Then the catalog directory should hold one catalog file per library
    And no catalog file should be named after a library name

  # Happy path
  Scenario: Organisation survives a restart
    Given an empty catalog directory
    And the owner created the library "Family archive" with the collection "Summer"
    When the catalog is closed and opened again
    Then the library "Family archive" should be at revision 2
    And the collection "Summer" should still be listed

  # Edge case
  Scenario: A retried command is answered once
    Given an empty catalog directory
    And the owner created the library "Family archive" with the collection "Summer"
    When the same collection command is retried after a restart
    Then the retry should answer the first result
    And the library "Family archive" should be at revision 2

  # Edge case
  Scenario: Journal entries survive an unreachable host journal
    Given an empty catalog directory whose host journal is unreachable
    When the owner creates the library "Family archive"
    And the host journal is reachable again when the catalog reopens
    Then the host journal should record "library.create"

  # Error case
  Scenario: A command that cannot be stored changes nothing
    Given an empty catalog directory
    And the owner created the library "Family archive" with the collection "Summer"
    When a commit with a membership of a missing file is applied
    Then the commit should fail
    And the library "Family archive" should be at revision 2

  # Edge case
  Scenario: A catalog is an index that a backup copies consistently
    Given an empty catalog directory
    And the owner created the library "Family archive" with the collection "Summer"
    When the owner backs up the catalog of "Family archive"
    Then the backup should open as a catalog at revision 2
    And the host journal should record "library.catalog.backup"
