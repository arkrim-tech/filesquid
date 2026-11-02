# API-S-009: Operation IDs on errors — every 4xx and 5xx ApiError the router answers carries
# operation_id: the envelope's operation ID for a command, else an opaque ID the router mints for
# that request in the operation-ID form of API-S-002 (the orchestrator's proposal, owner to
# confirm); the journal entry of that refusal or failure carries the same ID, so a client can show
# "{code} · op {id}" (mockup 25-panel-states.svg) and open the Logs view filtered to it.
# Owner decision 2026-11-02, verbatim: "make sure each API row issues gets a fix in MVP".
# Scenarios only; Task API-F2.33 adds their step definitions (unmatched steps are reported as skipped).
Feature: Operation IDs on error answers
  As a library owner reading an error in the file panel
  I want each error to name an operation I can look up in the Logs
  So that I can find out what went wrong

  # Happy path
  Scenario: A refused read names a minted operation ID that its journal entry carries
    Given error-ids file the caller may not read
    When error-ids caller reads its history
    Then error-ids answer should be refused with an operation_id
    And error-ids journal entry of that refusal should carry the same operation_id

  Scenario: A refused command names its envelope's operation ID
    When error-ids caller sends a command with a stale revision under the operation ID "op-stale-1"
    Then error-ids answer should carry operation_id "op-stale-1"

  # Edge cases
  Scenario: Two refused reads get two different operation IDs
    When error-ids caller makes two reads that are both refused
    Then error-ids answers should carry two different operation IDs

  Scenario: A failure inside the router carries an operation ID too
    Given error-ids store that fails while answering a read
    When error-ids caller makes that read
    Then error-ids answer should be HTTP 500 with an operation_id

  # Privacy
  Scenario: A minted operation ID encodes nothing about the request
    When error-ids caller reads a file by a path-like name and is refused
    Then error-ids operation_id should contain no path, name, token or content handle
