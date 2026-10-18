# F-036: Browser and mobile-web experience
# SRS-F-036: Browser and mobile-web experience (the replica, its cache identity and pending changes)
# API-S-024: Replica, cache identity and pending changes
# API-S-005: Responses carry opaque handles, never raw filesystem paths or SQL
# T-022: Browser storage is a cache, never canonical; cache loss produces clear recovery states
# ADR 008 and ADR 025: pending changes are canonical OCFL commits only once the fenced head publication succeeds
Feature: Replica, cache identity and pending changes
  As a person using FileSquid in a browser
  I want my local copy to show only what I may see, to say when it is incomplete or gone, and to keep my unpublished changes visibly pending
  So that a cache never shows another person's content, never hides a loss and never pretends an unpublished change is saved

  Background:
    Given a replica for viewer "alice" on library "family" with grants "file.read" and "file.write"

  # Happy path
  Scenario: A stored view answers with its coverage
    Given the replica stored a complete view for alice
    When alice reads the view from the replica
    Then the answer should say the coverage is complete

  Scenario: An incomplete view says so
    Given the replica stored a view with 3 of 8 index parts loaded
    When alice reads the view from the replica
    Then the answer should say 3 of 8 parts are loaded

  # Permission-aware identity
  Scenario: Another viewer never reads alice's stored view
    Given the replica stored a complete view for alice
    When bob reads the same view from the replica
    Then the replica should report "not_cached"

  Scenario: A changed grant makes the old stored view unreachable
    Given the replica stored a complete view for alice
    When alice's grants shrink to "file.read" and she reads the same view
    Then the replica should report "not_cached"

  Scenario: A stored view made for another identity is refused
    Given the replica holds a view made for bob under alice's key
    When alice reads the view from the replica
    Then the replica should report "identity_mismatch"

  # Cache loss is visible
  Scenario: An evicted view says it was evicted
    Given the replica stored a complete view for alice
    And the device evicts that view
    When alice reads the view from the replica
    Then the replica should report "evicted"
    And the answer should not be an empty page

  # Pending changes
  Scenario: A new change is pending, not canonical
    When alice hides a file while offline
    Then the change should be "queued"
    And library "family" should have 1 pending change

  Scenario: A change becomes canonical only after its fenced head publication
    Given alice has queued a change
    When the change begins publishing and the head publication succeeds at revision "4"
    Then the change should be "committed"
    And library "family" should have 0 pending changes

  Scenario: A lost race is a conflict that blocks later changes
    Given alice has queued two changes
    When the first change begins publishing and loses the race
    Then the first change should be "conflict"
    And the second change should not be allowed to publish

  Scenario: Repeated failures stall a change for the owner to decide
    Given alice has queued a change
    When the change fails to publish 5 times in a row
    Then the change should be "stalled"

  Scenario: Pending changes survive a reload
    Given alice has queued a change
    When the device reloads from the saved list of pending changes
    Then the change should be "queued"

  # Failure
  Scenario: A damaged saved list is refused
    When the device reloads from a damaged saved list
    Then the saved list should be refused as "invalid_snapshot"
