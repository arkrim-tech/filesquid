# API-S-011: per-user UI preferences, storage use, whole-file download and listings
# SRS-F-062 / PRD F-062: drive-style views, sorting, selection and layout preferences
# UI-S-017: navigation preferences kept per user through the API
# API-S-021: navigation node handles
Feature: Preferences, whole-file downloads and drive listings through the API
  As a FileSquid user
  I want my view choices to follow me, whole files to download at any size and listings to show only my files
  So that every host I sign in to looks the way I left it and nothing FileSquid keeps for itself gets in my way

  Scenario: A user who saved nothing gets the default preferences
    Given a drive host that keeps preferences
    When the user reads their preferences
    Then the preferences should show the "list" layout and 0 pinned destinations

  Scenario: Saved preferences come back and the journal keeps none of their values
    Given a drive host that keeps preferences
    When the user saves the "grid" layout with one pinned collection
    Then the preferences should show the "grid" layout and 1 pinned destinations
    And the save should be journaled once without its values

  Scenario: A navigation choice over its bound is refused
    Given a drive host that keeps preferences
    When the user saves 101 pinned destinations
    Then the drive answer should be the error "invalid_request"
    And no preferences should be kept for the user

  Scenario: A file larger than one content answer downloads whole
    Given a drive file "films/reel.bin" larger than one content answer
    When the user streams "films/reel.bin" whole
    Then the stream should carry every byte as an attachment named "reel.bin"

  Scenario: Listings leave out the folders FileSquid keeps for itself
    Given a drive object holding "a.txt", "provenance/v1.jsonld" and ".filesquid/labels.json"
    When the user lists the drive object's root folder
    Then the drive listing should show only "a.txt"
    And listing inside "provenance" should not be found

  Scenario: A folder node lists its folder of the collection's object
    Given the "Photos" collection is served as one OCFL object
    When the user lists the folder node "2025"
    Then the drive listing should show "2025/summer" and "2025/winter"

  Scenario: A library node does not list as a folder
    Given the "Photos" collection is served as one OCFL object
    When the user lists the library node
    Then the drive answer should be the error "invalid_request"
