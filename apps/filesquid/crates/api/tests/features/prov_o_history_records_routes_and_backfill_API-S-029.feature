# API-S-028: PROV-O record per commit and journal linkage
# API-S-029: History routes and backfill
# API-S-032: Durable journal (per-commit journal entries)
# SRS-F-067 / PRD F-067: PROV-O history of every version and change
# PRD F-046: privacy on export and sharing (redacted history export)
Feature: PROV-O records, history routes and backfill in filesquid-api
  Every commit the API drives writes, in the same OCFL version, the PROV-O record of that
  version and the journal entries of the commit. The history routes answer from an index
  rebuilt from those records, and a bounded backfill job reconstructs records for versions
  committed before the history existed.

  Scenario: A repaired object gets a provenance record under the repair run ID
    Given a library with an object whose head has no version inventory, recording provenance
    When the owner repairs that object
    Then a new version of the object should hold a PROV-O record of kind "repair"
    And the record, the consent and the outcome should share the repair run ID
    And the new version should carry its "ocfl.commit" journal entry under the ID the journal stored

  # Error case
  Scenario: Without a provenance context the repair says that no record was written
    Given a library with an object whose head has no version inventory, without provenance
    When the owner repairs that object
    Then the repair answer should name the provenance error "provenance_not_configured"
    And a "prov.record" warning should be journaled under the repair run ID

  Scenario: A backfill reconstructs every missing record once
    Given a library of 2 objects committed before the history existed
    When the history backfill runs
    Then every version should have a record and the earlier ones should be marked reconstructed
    And the backfill process should end "completed" with 2 of 2 objects done
    When the history backfill runs again
    Then the second run should commit nothing

  # Edge case
  Scenario: A backfill stops at its cap and the next run continues
    Given a library of 2 objects committed before the history existed
    When the history backfill runs with a cap of 3 records
    Then the run should report that it reached its cap
    When the history backfill runs again
    Then every version should have a record and the earlier ones should be marked reconstructed

  Scenario: The history of a file lists its activities in commit order
    Given a history holding 2 versions of one file and 1 version of another
    When the caller lists the history of the first file
    Then the answer should hold 2 entries of that file, oldest first

  # Edge case
  Scenario: The history is paged with an opaque cursor
    Given a history holding 2 versions of one file and 1 version of another
    When the caller lists the whole history 2 entries at a time
    Then the pages should hold 2 and then 1 entries and the last page no cursor

  Scenario: The record of one version is raw JSON-LD
    Given a history holding 2 versions of one file and 1 version of another
    When the caller reads the record of version "v2" of the first file
    Then the answer should be the JSON-LD record of that version

  # Error case
  Scenario: A version without a record is not found
    Given a history holding 2 versions of one file and 1 version of another
    When the caller reads the record of version "v9" of the first file
    Then the response should be an "not_found" error with a message

  # Error case
  Scenario: Two filter dimensions at once are refused
    Given a history holding 2 versions of one file and 1 version of another
    When the caller lists the history by object and by operation at once
    Then the response should be an "invalid_request" error with a message

  Scenario: A redacted export names no installation and no path
    Given a history holding 2 versions of one file and 1 version of another
    When the caller exports the history of the first file redacted
    Then the export should be a JSON-LD array of 2 records without the installation or any path

  # Error case
  Scenario: The history routes wait for the host to install its history
    Given a host that has not installed its history yet
    When the caller lists the history of the first file
    Then the response should be an "unavailable" error with a message
    And the error should be retryable

  # Not automated here: API-S-029's admin backfill route (POST /api/v1/admin/history/backfill)
  # waits on the hosts injecting their library storage root (DE-0.3 coordinator).
