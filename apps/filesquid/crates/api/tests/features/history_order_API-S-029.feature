# API-S-029: History routes — GET /api/v1/files/{id}/history and GET /api/v1/history take order
# oldest_first (the default) or newest_first, so a client reaches the latest activities of a
# history longer than its page bound first; a cursor of one order is refused as invalid_request
# under the other. The page bounds are unchanged.
# Owner decision 2026-11-02, verbatim: "make sure each API row issues gets a fix in MVP".
# Scenarios only; Task API-F2.33 adds their step definitions (unmatched steps are reported as skipped).
Feature: Newest-first history
  As a library owner
  I want a file's newest activities first
  So that a long history still shows what happened most recently

  # Happy path
  Scenario: Newest first answers the latest activity first
    Given history-order file with 3 activities
    When history-order owner reads its history newest first
    Then history-order activities should be in the reverse order of oldest first

  Scenario: A history longer than the page bound shows its newest activities
    Given history-order file with 5,001 activities
    When history-order owner reads one page of its history newest first
    Then history-order page should start with the 5,001st activity

  # Edge cases
  Scenario: Oldest first stays the default
    Given history-order file with 3 activities
    When history-order owner reads its history without an order
    Then history-order activities should be oldest first

  Scenario: Newest-first pages have no repeat or gap
    Given history-order file with 450 activities
    When history-order owner reads every page of its history newest first, 200 at a time
    Then history-order pages should hold all 450 activities once each, newest first

  # Error cases
  Scenario: A cursor of the other order is refused
    Given history-order cursor from an oldest-first page
    When history-order owner reads the next page newest first with that cursor
    Then history-order answer should be refused as invalid_request

  Scenario: The library-wide history keeps its permission filter newest first
    Given history-order library with activities on a file the caller may not read
    When history-order caller reads the library history newest first
    Then history-order answer should hold no activity of that file
