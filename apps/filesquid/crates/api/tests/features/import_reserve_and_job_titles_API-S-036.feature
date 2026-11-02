# API-S-036: Review — the plan carries reserve_bytes, the bytes the host keeps free on the library's
# volume (on the desktop the larger of 2 GiB and 5 % of its capacity; DE-S-019), so the review can
# say "… and keeps {reserve} free". One meaning — a recorded journal that cannot be read in full
# fails the replay of a resumed import, so the file is quarantined with its error code and never
# reported imported (DE-S-011). API-S-037: Per-file progress — an upload job's view carries its
# destination collection's current title (collection_title), null once that collection is gone.
# Owner decisions 2026-11-02, verbatim: "make sure each API row issues gets a fix in MVP" and
# "I accept your recommenations on the remaining issues" (keeping DE-0.10's quarantine).
# Scenarios only; Task API-F2.32 adds the step definitions of the reserve and title scenarios
# (unmatched steps are reported as skipped); DE-0.10's unit tests already prove the quarantine
# (native unit_import_file).
Feature: Import plan reserve, job destination titles and replay quarantine
  As a library owner uploading files
  I want the review to say how much space stays free and every job to name its collection
  So that I know what an upload will leave on my disk and where each job's files go

  # Happy path
  Scenario: The plan names the reserve the job keeps
    Given import-facts library on a volume of 1 TB capacity
    When import-facts owner plans an upload into the collection "Photos 2026"
    Then import-facts plan should carry reserve_bytes of 5 % of 1 TB
    And import-facts reserve should equal the free space the import job keeps

  Scenario: A job names its destination collection in a new session
    Given import-facts upload job into the collection "Photos 2026" started in an earlier session
    When import-facts owner reads the job
    Then import-facts job destination should carry collection_title "Photos 2026"

  # Edge cases
  Scenario: A small volume keeps 2 GiB
    Given import-facts library on a volume of 20 GiB capacity
    When import-facts owner plans an upload into the collection "Photos 2026"
    Then import-facts plan should carry reserve_bytes of 2 GiB

  Scenario: A renamed collection is named by its current title
    Given import-facts upload job into the collection "Photos 2026", renamed "Summer 2026" since
    When import-facts owner reads the job
    Then import-facts job destination should carry collection_title "Summer 2026"

  Scenario: A deleted destination collection has no title
    Given import-facts upload job whose destination collection was deleted
    When import-facts owner reads the job
    Then import-facts job destination should carry a null collection_title

  # Error and privacy cases
  Scenario: A collection title never reaches the journal
    Given import-facts upload job into the collection "Tax 2026"
    When import-facts owner reads, pauses and resumes the job
    Then import-facts journal should not contain "Tax 2026"

  Scenario: A resumed import whose recorded journal cannot be read is quarantined
    Given import-facts import committed before a crash whose recorded journal cannot be read in full
    When import-facts resumed job replays that import
    Then import-facts file should be failed with its error code and kept for the owner's retry
    And import-facts file should not be reported as imported
