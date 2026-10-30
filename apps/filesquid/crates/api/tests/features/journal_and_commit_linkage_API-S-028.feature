# API-S-028: PROV-O record per commit and journal linkage
# API-S-017: Activity journal entry shape and redaction
# ADR 040 Decision 6: Prerequisites (journal and commit linkage)
Feature: Journal and commit linkage of the file and metadata routes
  As a library owner
  I want every version a file or metadata route commits to name its operation and my installation, and to be journaled once
  So that the activity journal and the version history join on identifiers, never on timestamps

  Background:
    Given an owner's library with provenance configured for a known installation
    And the library holds an object with an edited file and an uploaded file

  # Happy path
  Scenario Outline: Each file and metadata route commit is linked to the journal
    When the owner <operation> with a new operation ID
    Then exactly one "ocfl.commit" entry should name that operation ID and the new object version
    And the new version's user should be the installation and its message should name the operation ID and the kind "<kind>"
    And the new version should hold its PROV-O record

    Examples:
      | operation                                 | kind          |
      | uploads a new version of the uploaded file | upload        |
      | copies the uploaded file                  | copy          |
      | renames the uploaded file                 | rename        |
      | moves the edited file                     | move          |
      | removes the edited file                   | delete        |
      | restores the edited file's first version  | reinstate     |
      | sets a statement on the uploaded file     | metadata.edit |

  Scenario: Every metadata write is journaled as metadata.edit without its value
    When the owner sets a statement on the uploaded file with a new operation ID
    Then one "metadata.edit" entry should name that operation ID and the new object version
    And no journal entry should contain the statement's value

  # Edge case
  Scenario: The caller's message is the version label
    When the owner moves the edited file with the message "Tidy up"
    Then the new version's message should carry the label "Tidy up"

  # Error case
  Scenario: A refused write is journaled as before and never reaches the history
    When the owner moves the edited file naming a stale revision
    Then the move should be journaled once as refused with its operation ID
    And no "ocfl.commit" entry and no new version should exist for that operation
