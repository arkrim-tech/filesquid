# API-S-002: Every mutation route, the file write routes included (upload, create folder, rename
# and move, copy, remove, restore, version labels), MUST take a filesquid-core CommandEnvelope and
# answer a CommandOutcome for the same operation ID; a conflict MUST be HTTP 409 with a conflict
# outcome, and a repeated operation ID of a command that changed something MUST answer its first
# outcome with no second change.
# API-S-019: a command whose expected revision is not the current revision is refused as conflict
# with no change; the stored receipt is answered before the revision is checked; a command that
# changes nothing stores no receipt.
# UI-S-012: a 2xx answer resolves to a CommandOutcome for the same operation id; HTTP 409
# resolves to conflict.
# Task API-F2.13 (contract 0.25.0).
Feature: Command envelopes on the file write routes
  As the owner of a library
  I want every file change to carry an operation ID and the revision I last read
  So that a retried request never changes my files twice and a stale one changes nothing

  Background:
    Given the owner's library holds an object at its first version

  # Happy path
  Scenario: A move carries an envelope and answers a committed outcome for the same operation
    When the owner moves "a.txt" to "c.txt" as operation "op-move-1" expecting "v1"
    Then the answer should be a committed outcome of operation "op-move-1"
    And its result should name version "v2"
    And the journal should record one "file.move" success under operation "op-move-1"

  Scenario: An upload is a multipart command with its envelope, input and file
    When the owner uploads "notes.txt" as operation "op-upload-1" expecting "0"
    Then the answer should be a committed outcome of operation "op-upload-1"
    And its result should name a new file at version "v1"

  # Edge cases
  Scenario: A repeated operation answers its first outcome with no second version
    Given the owner moved "a.txt" to "c.txt" as operation "op-move-2" expecting "v1"
    When the owner moves "a.txt" to "c.txt" as operation "op-move-2" expecting "v1"
    Then the answer should be the first answer of operation "op-move-2"
    And the object's head should be "v2"
    And the journal should record one "file.move" success under operation "op-move-2"

  Scenario: A stale expected revision is a conflict outcome and changes nothing
    When the owner removes "b.txt" as operation "op-remove-1" expecting "v7"
    Then the answer should be HTTP 409 with a conflict outcome of operation "op-remove-1"
    And the object's head should be "v1"

  Scenario: Creating a folder stores no receipt, so its retry is evaluated again
    Given the owner created the folder "albums" as operation "op-folder-1" expecting "v1"
    And a file "albums/x.txt" is then committed beneath that folder
    When the owner creates the folder "albums" as operation "op-folder-1" expecting "v2"
    Then the answer should be a committed outcome of operation "op-folder-1"
    And its result should say the folder is no longer pending

  # Error cases
  Scenario: A write without an envelope is refused and journaled once
    When the owner sends a removal of "b.txt" without an envelope
    Then the command should be refused with "invalid_request"
    And the object's head should be "v1"
    And the journal should record one "files.remove" refusal

  Scenario: An operation ID of another command is refused
    Given the owner moved "a.txt" to "c.txt" as operation "op-move-3" expecting "v1"
    When the owner removes "b.txt" as operation "op-move-3" expecting "v2"
    Then the command should be refused with "invalid_request"
    And the object's head should be "v2"
