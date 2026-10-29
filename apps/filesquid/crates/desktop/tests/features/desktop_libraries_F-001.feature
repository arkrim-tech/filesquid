# F-001: Independent local library and ownership
# SRS-F-001: Independent local library and ownership
# API-S-025: Local library and repair routes: hosts inject the storage root and the repair ledger; answers carry opaque identifiers, never a path; the repair is the desktop owner's alone and needs the owner's consent
# DE-S-006: The webview receives opaque handles and API responses, never raw filesystem paths or SQL
# DE-S-018: Library routes over owner-chosen folders: opaque location IDs, shared leases, a persistent repair ledger, and each bound library served
# DE-S-009: Each library's catalog index lives in the data directory, keyed by the library's UUID; the library itself lives only in the folder the owner chose
# DE-S-010: A missing index, one of another copy or one behind the snapshot head is rebuilt from the folder in one journaled reconcile
Feature: Desktop libraries in folders the owner chose
  As the owner using the FileSquid desktop app
  I want to create, open, copy and repair my libraries in folders I choose
  So that my library stays mine, in ordinary folders, and no screen ever learns where they are

  Background:
    Given the desktop host has started its library routes

  # Happy path
  Scenario: A library created in an empty chosen folder gets an opaque ID
    Given the owner chose an empty folder
    When the app creates a library in that folder
    Then the answer should name a new library without any folder path
    And the desktop should serve that library's files

  Scenario: A completed snapshot is copied into another empty folder
    Given the owner opened a folder that holds a library
    And the owner chose an empty folder
    When the app copies the library's snapshot into that folder
    Then the copy should succeed with no invalid object
    And the chosen folder should hold an OCFL storage root

  Scenario: The data directory keeps only the library's index, never its files
    Given the owner chose an empty folder
    When the app creates a library in that folder
    Then the data directory should hold only that library's index
    And the chosen folder should hold the library's collection snapshot

  Scenario: A library folder copied elsewhere reopens with its collections
    Given a library folder holding the collection "Holidays" was copied to a new place
    When the app opens the copied folder without naming its library
    Then the answer should name the library the copied folder holds
    And the library should list the collection "Holidays"

  # Edge cases
  Scenario: A second folder of a library that is already open is refused
    Given the owner opened a folder that holds a library
    And a copy of that folder was chosen
    When the app opens the copied folder without naming its library
    Then the request should be refused as a conflict

  Scenario: A folder that already holds a file is not used for a new library
    Given the owner chose a folder that holds a file
    When the app creates a library in that folder
    Then the request should be refused as a conflict
    And the folder should still hold only that file

  Scenario: A snapshot is never copied into a folder inside the library
    Given the owner opened a folder that holds a library
    And the owner chose a folder inside that library
    When the app copies the library's snapshot into that folder
    Then the request should be refused as a conflict

  # Error cases
  Scenario: A repair without the owner's consent changes nothing
    Given the owner opened a folder that holds a library whose head lacks its version inventory
    When the app asks to repair it without the owner's consent
    Then the request should be refused as a conflict
    And the head should still lack its version inventory

  Scenario: An unknown folder is refused and journaled once without a path
    When the app creates a library in a folder the host never registered
    Then the request should be refused as not found
    And the journal should record the refusal once without any folder path

  # Owner interaction
  Scenario: The owner's consented repair is kept after the app restarts
    Given the owner opened a folder that holds a library whose head lacks its version inventory
    When the owner confirms the repair of that library
    Then the repair should succeed
    And after the app restarts the repair ledger should still hold its consent and receipt
