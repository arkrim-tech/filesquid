# API-S-002: Every mutation route MUST take a filesquid-core CommandEnvelope and answer a
# CommandOutcome for the same operation ID; a conflict MUST be HTTP 409 with a conflict outcome,
# and a repeated operation ID of a command that changed something MUST answer its first outcome
# with no second change. The library mutation routes (create, open, copy, rebuild, rename) take
# the envelope (owner decision 1 of 2026-10-23); the location-choices route and the repair route
# keep their bodies.
# API-S-019: create-library sends expected revision 0; a command whose expected revision is not
# the library's current revision is refused as conflict with no change; the stored receipt is
# answered before the revision is checked; a command that changes nothing stores no receipt.
# API-S-025: the rebuild runs as a durable job answered pending; a request for a library whose
# rebuild job is unfinished answers that job and submits none.
# API-S-038: the rename route takes the envelope; a changed name advances the library's revision.
# UI-S-012: a 2xx answer resolves to a CommandOutcome for the same operation id; HTTP 409
# resolves to conflict.
# Task API-F2.19.
Feature: Command envelopes on the library routes
  As the owner of a library
  I want every library command to carry an operation ID and the revision I last read
  So that a retried request never changes my library twice and a stale one changes nothing

  # Happy path
  Scenario: Creating a library carries an envelope and answers a committed outcome
    Given the desktop owner's host with an empty folder registered
    When the owner creates a library in that folder as operation "op-create-1" expecting "0"
    Then the answer should be a committed library outcome of operation "op-create-1"
    And the library should be at revision "1"
    And the registered folder should hold only the OCFL 1.1 storage-root declaration

  Scenario: The rebuild answers its durable job as the pending outcome
    Given the desktop owner's library with a complete object and an object without a version inventory, on a host that runs no rebuild yet
    When the owner starts a rebuild of the library as operation "op-rebuild-1" expecting "1"
    Then the answer should be a pending library.rebuild job without a result
    And the answer should be a pending outcome of operation "op-rebuild-1" naming the same job

  Scenario: Renaming a library carries an envelope and advances its revision
    Given the desktop owner's library with a complete object and an object without a version inventory
    When the owner renames the library to "Family archive" as operation "op-rename-1" expecting "1"
    Then the answer should be a committed library outcome of operation "op-rename-1"
    And the library should be at revision "2"
    And "library.rename" should be journaled 1 time as a success

  # Edge cases
  Scenario: A repeated copy answers its first answer with no second write
    Given the desktop owner's library with a complete object and an object without a version inventory
    When the owner copies the library to an empty folder as operation "op-copy-1" expecting "1"
    And the owner copies the library to an empty folder as operation "op-copy-1" expecting "1"
    Then the library answer should be the first answer of operation "op-copy-1"
    And "library.snapshot.copy" should be journaled 1 time as a success
    And the library folder should be unchanged

  Scenario: A repeated rebuild request answers the same unfinished job
    Given the desktop owner's library with a complete object and an object without a version inventory, on a host that runs no rebuild yet
    When the owner starts a rebuild of the library as operation "op-rebuild-2" expecting "1"
    And the owner starts a rebuild of the library as operation "op-rebuild-3" expecting "1"
    Then the answer should be a pending outcome of operation "op-rebuild-3" naming the same job
    And "job.created" should be journaled 1 time as a success

  Scenario: A stale expected revision is a conflict outcome and changes nothing
    Given the desktop owner's library with a complete object and an object without a version inventory
    When the owner copies the library to an empty folder as operation "op-copy-2" expecting "9"
    Then the library answer should be HTTP 409 with a conflict outcome of operation "op-copy-2"
    And the copy folder should be empty
    And exactly one refusal "conflict" should be journaled for "library.snapshot.copy"

  # Error cases
  Scenario: A library command without an envelope is refused and journaled once
    Given the desktop owner's library with a complete object and an object without a version inventory
    When the owner sends a rebuild of the library without an envelope
    Then the answer should be the error "invalid_request"
    And exactly one refusal "invalid_request" should be journaled for "library.rebuild"
    And the library folder should be unchanged

  Scenario: The repair route keeps its consent body
    Given the desktop owner's library with a complete object and an object without a version inventory
    When the owner repairs the object without a version inventory with consent through the API
    Then the object should be answered as repaired
