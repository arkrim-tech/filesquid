# OCFL-S-018: Write order and unpublished versions. The commit receipt reports whether the commit took over an expired, unreleased reservation, so the host can validate the object at once. `vN` MUST be reserved before any `vN` byte through an exclusive, fenced lease or reservation; a writer whose lease expired or was reassigned MUST NOT write content or publish the head; content, then version inventory and sidecar, then root inventory and sidecar last; an incomplete `vN` is unpublished.
# OCFL-S-023: Takeover reporting (every commit receipt reports whether its reservation took over an unreleased, expired one)
# F-042: OCFL preservation and portable recovery (writes serialize per object and recover interrupted publication)
# NFR-001: Preservation and transactional integrity (zero silent loss, zero false-complete versions)
Feature: Local write order and unpublished versions
  As a preservation programme writing OCFL objects with filesquid-ocfl
  I want every new version reserved before its bytes and published by the root inventory last
  So that a crash or a stale writer never leaves a false, partial or overwritten version

  Background:
    Given an OCFL object "obj" committed at "v1" through the engine writer

  # Happy path
  Scenario: A new version is published in write order
    When writer "writer-a" commits "v2" adding "d.txt"
    Then the commit receipt should report no reservation takeover
    And the reservation should come before every "v2" byte
    And the writes should end with the version inventory, the root inventory and the root sidecar
    And the object head should be "v2"
    And the object should validate with zero errors

  Scenario: Unchanged and renamed files reuse their stored bytes
    When writer "writer-a" commits "v2" renaming "b.txt" to "c.txt"
    Then no content should be written under "v2"
    And "c.txt" should resolve to the content path "v1/content/b.txt"

  # Edge cases: a crash at each step of publication
  Scenario Outline: A crash leaves the new version unpublished until it is recovered
    Given writer "writer-a" crashed while writing "<key>" during the commit of "v2"
    When I open the written object
    Then the object head should be "v1"
    And pinning "v2" should be refused as an unknown version
    When the crashed writer's lease lapses and writer "writer-b" retries the commit
    Then the object head should be "v2"
    And the commit receipt should report a reservation takeover
    And the history should list "v1, v2"
    And the object should validate with zero errors

    Examples:
      | key                          |
      | obj/v2/content/d.txt         |
      | obj/v2/inventory.json.sha256 |
      | obj/inventory.json           |
      | obj/inventory.json.sha256    |

  # Error cases: held reservations, stale holders and a moved head
  Scenario: A second writer cannot reserve a version that is held
    Given writer "writer-b" holds the reservation for "v2"
    When writer "writer-a" commits "v2" adding "d.txt"
    Then the commit should be refused because the reservation is held
    And no byte should be written under "v2"

  Scenario: A stale lease holder cannot write content after reassignment
    Given the lease passes to writer "writer-b" after "obj/v2/content/d.txt" is written
    When writer "writer-a" commits "v2" adding "d.txt" and "e.txt"
    Then the commit should be refused because the reservation was lost
    And "obj/v2/content/e.txt" should not be stored
    And the object head should be "v1"

  Scenario: A stale lease holder cannot publish the head after reassignment
    Given the lease passes to writer "writer-b" after "obj/v2/inventory.json.sha256" is written
    When writer "writer-a" commits "v2" adding "d.txt"
    Then the commit should be refused because the reservation was lost
    And the root inventory should be unchanged
    And the object head should be "v1"

  Scenario: A moved head is refused without publishing
    Given writer "writer-b" has committed "v2" adding "d.txt"
    When writer "writer-a" commits on top of "v1" adding "e.txt"
    Then the commit should be refused because the head moved to "v2"
    And the object head should be "v2"
