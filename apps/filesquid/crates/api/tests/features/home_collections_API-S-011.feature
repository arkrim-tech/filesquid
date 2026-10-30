# API-S-011: Destination — every upload, copy and move MUST name collection_id (and MAY name a
# folder); a request without it MUST be refused as destination_required before any byte is
# staged, an unknown or foreign collection as collection_not_found; each upload and copy MUST
# register the file in the collection snapshot at <folder>/<name> under the request's
# operation ID (API-S-026), journaled once with opaque IDs, raising navigation.change
# (API-S-021). Same logical path (ADR 041 Decision 12): a taken path, a recoverably deleted
# file's included, MUST be numbered beside it, never refused or overwritten; after 9,999
# numbers the request MUST be refused as path_suffix_exhausted with no change. A crash between
# the file's object and its registration MUST resume once under the same operation ID
# (API-S-019).
# Task API-F2.14 (contract 0.33.0).
Feature: Home collections for uploads and copies
  As a library owner
  I want every file I upload or copy to land in the collection and folder I chose
  So that it appears there at once and never overwrites a file that is already there

  Background:
    Given a library whose collection "Holidays" receives uploads

  # Happy path
  Scenario: An upload lands in the chosen folder of the collection
    When the owner uploads "beach.jpg" into the folder "Summer"
    Then the file should be registered at "Summer/beach.jpg" in "Holidays"
    And the answer should name the collection and the path "Summer/beach.jpg"
    And the upload should be journaled once without its name or folder
    And the folder should be announced as changed

  Scenario: A copy lands beside its source with a numbered name
    Given the owner has uploaded "beach.jpg" into the folder "Summer"
    When the owner copies it into the folder "Summer"
    Then the copy should be registered at "Summer/beach-1.jpg" in "Holidays"

  # Edge cases
  Scenario Outline: A taken path is numbered beside the existing item
    Given the path "<taken>" is already taken in "Holidays"
    When the owner uploads "<name>" into the collection's root
    Then the file should be registered at "<numbered>" in "Holidays"

    Examples:
      | taken           | name            | numbered          |
      | photo.jpg       | photo.jpg       | photo-1.jpg       |
      | archive.tar.gz  | archive.tar.gz  | archive-1.tar.gz  |
      | report          | report          | report-1          |
      | .env            | .env            | .env-1            |
      | .config.json    | .config.json    | .config-1.json    |

  Scenario: A recoverably deleted file keeps its place
    Given a recoverably deleted file holds the path "notes.txt" in "Holidays"
    When the owner uploads "notes.txt" into the collection's root
    Then the file should be registered at "notes-1.txt" in "Holidays"

  Scenario: A retry after a crash completes the same file once
    Given an upload of "crash.txt" committed its file but crashed before its registration
    When the owner retries the upload with the same operation ID
    Then the retry should answer the same file at its first version
    And the file should be registered at "crash.txt" in "Holidays"

  # Error cases
  Scenario: An upload without a collection is refused before anything is staged
    When the owner uploads "beach.jpg" without naming a collection
    Then the upload should be refused as "destination_required" and journaled once
    And nothing should be registered

  Scenario: An upload into a collection of no library here is refused
    When the owner uploads "beach.jpg" into a collection this library does not hold
    Then the upload should be refused as "collection_not_found" and journaled once
    And nothing should be registered

  Scenario: After 9,999 numbers the upload is refused and nothing changes
    Given the path "full.txt" and every number up to 9999 beside it are taken in "Holidays"
    When the owner uploads "full.txt" into the collection's root
    Then the upload should be refused as "path_suffix_exhausted" and journaled once
    And nothing should be registered
