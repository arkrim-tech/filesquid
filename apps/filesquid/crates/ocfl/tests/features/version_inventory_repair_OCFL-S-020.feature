# OCFL-S-020: Version-inventory repair. A consented, one-time repair gives a committed head without a version inventory (W010) a byte-identical copy of the root inventory and root sidecar, under the same fenced reservation as a commit; every refusal check runs before any write; an interrupted repair leaves the logical state unchanged and is completed by running it again; no version is published.
# OCFL-S-018: Write order and unpublished versions (a head without its version inventory is refused as the base of a commit until it is repaired)
# OCFL-S-019: Inspection and copy never edit an object (the repair is the one exception, and only when the caller runs it)
# F-042: OCFL preservation and portable recovery
# NFR-001: Preservation and transactional integrity (zero silent loss, zero false-complete versions)
Feature: Version-inventory repair for heads without one
  As a preservation programme that keeps OCFL objects written by other tools
  I want a consented repair that adds the missing head version inventory and nothing else
  So that such objects become editable without any change to their content or history

  # Happy path
  Scenario: The pinned W010 fixture is repaired and stays closed to commits as sha512
    Given a copy of the pinned W010 fixture object
    And a commit on it is refused for the missing version inventory
    When the version-inventory repair runs
    Then the repair should have written "v1/inventory.json" and "v1/inventory.json.sha512"
    And the repaired head files should be byte-identical to the root inventory and root sidecar
    And no other file of the object should have changed
    And the repaired object should validate with zero errors and without W010
    And a commit on the repaired object should be refused as not sha256 without writing

  Scenario: A repaired sha256 object becomes editable
    Given a sha256 object committed at "v2" whose head version inventory was removed
    When the version-inventory repair runs
    Then the repair should have written "v2/inventory.json" and "v2/inventory.json.sha256"
    And the logical state should be unchanged by the repair
    And the repaired object should validate with zero errors and without W010
    And a commit of "v3" on the repaired object should succeed

  # Error cases: refused before any write
  Scenario Outline: The repair refuses an object it must not touch
    Given a sha256 object committed at "v2" whose head version inventory was removed
    And the object was then changed by <damage>
    When the version-inventory repair runs
    Then the repair should be refused as <refusal>
    And the repair should have written nothing

    Examples:
      | damage                                         | refusal                 |
      | restoring the head version inventory           | nothing to repair       |
      | a content file whose digest no longer matches   | invalid                 |
      | a root sidecar that does not match              | a root sidecar mismatch |
      | an unlisted file in the head version directory  | not repairable          |
      | a head version inventory that differs           | not repairable          |
      | a reservation held by another writer            | a held reservation      |

  Scenario: A repair whose reservation is lost writes nothing more
    Given a sha256 object committed at "v2" whose head version inventory was removed
    And the reservation passes to another writer after "obj/v2/inventory.json" is written
    When the version-inventory repair runs
    Then the repair should be refused as a lost reservation
    And "obj/v2/inventory.json.sha256" should not be in the repaired object
    And the logical state should be unchanged by the repair

  # Edge cases: a crash between the version inventory and its sidecar
  Scenario: An interrupted repair is completed by running it again
    Given a sha256 object committed at "v2" whose head version inventory was removed
    And a repair crashed before writing "obj/v2/inventory.json.sha256"
    Then the logical state should be unchanged by the repair
    And a commit of "v3" on the repaired object should be refused for the missing head version sidecar
    When the version-inventory repair runs again
    Then the repair should have written only "v2/inventory.json.sha256"
    And the repaired object should validate with zero errors and without W010
    And a commit of "v3" on the repaired object should succeed

  Scenario Outline: An interrupted repair on a damaged object is refused
    Given a sha256 object committed at "v2" whose head version inventory was removed
    And a repair crashed before writing "obj/v2/inventory.json.sha256"
    And the object was then changed by <damage>
    When the version-inventory repair runs again
    Then the repair should be refused as <refusal>
    And the repair should have written nothing

    Examples:
      | damage                                         | refusal                 |
      | a content file whose digest no longer matches   | invalid                 |
      | a root sidecar that does not match              | a root sidecar mismatch |
      | an unlisted file in the head version directory  | not repairable          |
      | a head version inventory that differs           | not repairable          |
      | a reservation held by another writer            | a held reservation      |

  # Inspection never repairs
  Scenario: Opening, reading and validating a W010 object never repairs it
    Given a copy of the pinned W010 fixture object
    When the object is opened, read and validated
    Then nothing should have been written to the storage root
