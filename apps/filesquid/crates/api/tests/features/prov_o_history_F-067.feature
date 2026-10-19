# F-067: PROV-O history of every version and change
# SRS-F-067: PROV-O history of every version and change
# API-S-028: PROV-O record per commit and journal linkage
# API-S-029: History routes and backfill
# Scenarios only; step definitions arrive with Task FS-2.8 (unbound steps are skipped by the harness).
Feature: PROV-O history of every version and change
  As a library owner
  I want a standards-based record of everything that happened to every file and its metadata
  So that the history survives copying and recovery without FileSquid

  Background:
    Given a desktop library with no c2pa module
    And the installation UUID of this FileSquid installation is known

  # Happy path
  # Exports, scrubbed outputs and publications are recorded in the library's history object (scenario below).
  Scenario Outline: Each file-changing operation commits exactly one PROV-O record in its own version
    Given a file in the library
    When I <operation> it
    Then exactly one new OCFL version should be committed
    And that version should contain its PROV-O record named after the new version under "provenance/"
    And the record should hold exactly one prov:Activity of kind "<kind>" with the operation ID and the object@version
    And the record should name the journal entry IDs of that commit

    Examples:
      | operation          | kind           |
      | import             | import         |
      | upload             | upload         |
      | copy               | copy           |
      | unhide             | unhide         |
      | reinstate          | reinstate      |
      | label              | label          |
      | add to a collection | catalog.organisation |
      | rename             | rename         |
      | move               | move           |
      | hide               | hide           |
      | delete             | delete         |
      | restore            | restore        |
      | purge              | purge          |
      | edit the metadata  | metadata.edit  |
      | revert             | revert         |

  Scenario: Entities and relations describe the change
    Given a file whose metadata record has one earlier version
    When I edit its metadata
    Then the new metadata state should be a prov:Entity with object ID, version, logical path and SHA-256
    And it should be prov:wasGeneratedBy the activity and prov:wasDerivedFrom the previous metadata state

  Scenario: A purge invalidates the removed state
    Given a deleted file
    When I purge it
    Then the removed state should be prov:wasInvalidatedBy the purge activity

  Scenario: The owner is the installation agent, not a person
    When I import a file
    Then the activity should be prov:wasAssociatedWith the agent "urn:uuid:" followed by the installation UUID
    And the record should contain no prov:Person and no display name
    And a prov:SoftwareAgent should name the FileSquid host and its version
    And the OCFL version user address should be the same installation URN

  Scenario: The installation UUID is the same across restarts
    Given I imported a file
    When I restart FileSquid and rename the file
    Then both activities should name the same installation agent

  Scenario: A server account is its own agent acting on behalf of the installation
    Given a filesquid-server installation with the account "ana"
    When "ana" renames a file
    Then the activity should be prov:wasAssociatedWith the agent "urn:uuid:" followed by the account UUID of "ana"
    And that account agent should prov:actedOnBehalfOf the server's installation agent
    And the record should contain no prov:Person, user name, e-mail address or display name

  Scenario: Journal and PROV-O join without timestamps
    When I rename a file
    Then the journal should hold one "ocfl.commit" entry with the operation ID and object reference of the file object and the new version
    And the PROV-O activity should list that entry's ID

  # Edge cases
  Scenario: A no-op save produces no version and no record
    Given a file whose metadata is unchanged
    When I save its metadata without changes
    Then no OCFL version should be committed
    And no PROV-O record should be written

  Scenario: A refused attempt stays in the journal only
    When a caller without the per-launch token tries to rename a file
    Then the journal should record the refusal
    And no PROV-O record should be written

  Scenario Outline: Exports, scrubbed outputs and publications are recorded in the library's history object
    When I <action>
    Then the library's history object should gain one provenance-only version with a "<kind>" activity
    And no file object should gain a version

    Examples:
      | action                                   | kind    |
      | export a folder as a ZIP download        | export  |
      | export a file with metadata scrubbing    | scrub   |
      | publish a public file to my AT Protocol account | publish |

  Scenario: A single-file download is journaled only
    When I download one file
    Then the journal should record a "file.download" entry with the file ID and version
    And no PROV-O record should be written
    And no OCFL version should be committed

  Scenario: A repair is followed by a provenance-only version
    Given an object whose head has no version inventory
    When the owner consents to the repair and it completes
    Then the repaired object should gain one provenance-only version whose activity names the repair run ID

  Scenario: A rebased commit regenerates its record
    Given another writer committed to the same object after my commit was staged
    When my commit is rebased onto the new head
    Then the record should name the rebased version and its inventory created time

  Scenario: Versions committed before the history existed are reconstructed once
    Given an object with three versions and no PROV-O records
    When the reconstruction job runs, is interrupted and resumes
    Then exactly one provenance-only version should be committed to the object
    And each of the three versions should have exactly one record marked "reconstructed"
    And each record should state its own recording time
    And no agent should appear that the inventory did not record
    And running the job again should commit nothing

  # Error case
  Scenario: A crash between publication and the journal write leaves no dangling link
    Given a commit whose OCFL version is published
    When the host stops before the journal entries are written
    And the library is opened again
    Then the journal should hold the entries named in the version's PROV-O record, with the same IDs

  Scenario: The history survives an ordinary copy
    Given a library with recorded history
    When I close FileSquid and copy the storage root with ordinary file tools
    Then a JSON parser alone should read every activity, entity and agent from the copy

  Scenario: Shared PROV-O is redacted
    Given a file with recorded history that includes its original file name
    When I share the file with history selected
    Then the shared history should contain no original file name, title or agent identity
    And the preview should say that the history was redacted

  # Source
  # - resources/srs.md SRS-F-067: PROV-O history of every version and change
  # - resources/prd.md F-067: PROV-O history of every version and change
  # - apps/filesquid/crates/api/resources/srs.md API-S-028, API-S-029
