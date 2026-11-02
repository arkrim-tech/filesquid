# API-S-019: Create-collection route — POST /api/v1/libraries/collections (action files.write) takes a
# CommandEnvelope of scope library:<library_id> with the library's current revision and the input
# {library_id, title}, and answers a CommandOutcome whose committed result is {collection_id, title};
# one snapshot version with its PROV-O activity, journaled as library.collection.create with opaque
# IDs only, announced by navigation.change. It is the command the default UI's first use (UI-S-034)
# already sends (apps/filesquid/ui/resources/tasks/Task-UI-1.3-Conflict.md, conflict 2).
# Owner decision 2026-11-02, verbatim: "make sure each API row issues gets a fix in MVP".
# Scenarios only; Task API-F2.32 adds their step definitions (unmatched steps are reported as skipped).
Feature: Create a collection through the API
  As a library owner
  I want to create a collection in my library
  So that my first library has somewhere to upload into, and I can add collections later

  Background:
    Given create-collection library "Family archive" opened by this host at its current revision

  # Happy path
  Scenario: The first collection of a new library is created
    When create-collection caller sends the command with the title "untitled collection"
    Then create-collection answer should be committed with a collection_id and the title "untitled collection"
    And create-collection library should gain one snapshot version with its PROV-O activity
    And create-collection command should be journaled once as library.collection.create with opaque IDs only
    And create-collection caller should receive one navigation.change event

  Scenario: A repeated operation ID answers the first outcome
    Given create-collection caller created "Drawings" under one operation ID
    When create-collection caller sends the same command again
    Then create-collection answer should be the first outcome with no second collection

  # Edge case
  Scenario: The UI's first-use command is accepted as it is sent
    When create-collection caller sends exactly the command the default UI's first use sends
    Then create-collection answer should be committed

  # Error cases
  Scenario: A stale revision changes nothing
    Given create-collection library moved on after the caller read its revision
    When create-collection caller sends the command with the title "Drawings"
    Then create-collection answer should be HTTP 409 with a conflict outcome
    And create-collection library should be unchanged

  Scenario Outline: A malformed command is refused
    When create-collection caller sends <command>
    Then create-collection answer should be refused as <code>

    Examples:
      | command                                     | code                      |
      | a blank title                               | invalid_request           |
      | a title of 1025 bytes                       | invalid_request           |
      | a scope naming another library              | invalid_request           |
      | a library this host has not opened          | not_found library_not_found |

  Scenario: A host without a library registry is unavailable
    Given create-collection host has no library registry
    When create-collection caller sends the command with the title "Drawings"
    Then create-collection answer should be HTTP 503 unavailable
