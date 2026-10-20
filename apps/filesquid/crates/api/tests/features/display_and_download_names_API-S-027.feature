# API-S-027: Display and download names
# API-S-026: the stored name is fixed; API-S-005: one safe display transformation
# PRD F-002, F-010, F-046 / C-033: original names everywhere, FileId names when sharing
Feature: Display and download names
  As the owner of a FileSquid library
  I want to see and download my files under their own names, and share them under neutral ones
  So that a rename never moves a stored file and sharing reveals nothing of my file names

  Scenario: The history and versions of a file carry its display name
    Given the owner has uploaded "Été à Nice.jpg"
    Then the file's history should name it "Été à Nice.jpg"
    And the file's versions should name it "Été à Nice.jpg"

  Scenario: A rename changes only the display name, in one new version
    Given the owner has uploaded "IMG_0001.JPG"
    When the owner renames the file to "Harbour at dawn.JPG"
    Then the rename should publish version "v2"
    And the listing should show "Harbour at dawn.JPG" at the same stored path
    And the facts should still hold the original name "IMG_0001.JPG"
    And the rename should be journaled as "files.rename" with opaque IDs only

  Scenario: A move of a file's stored original is refused
    Given the owner has uploaded "a.txt"
    When the owner moves the file's original to "b.txt"
    Then the move should be refused with a pointer to the rename route

  Scenario: A download name keeps quotes, backslashes and semicolons out of the fallback
    Given the owner has uploaded "summer.jpg"
    When the owner renames the file to "Sum\"mer\\2023;1.jpg"
    Then the download should be named "attachment; filename=\"Sum_mer_2023_1.jpg\"; filename*=UTF-8''Sum%22mer_2023%3B1.jpg"

  Scenario: A name without an ASCII letter or digit falls back to the file identifier
    Given the owner has uploaded "summer.jpg"
    When the owner renames the file to "写真 — 夏"
    Then the download fallback should be the file identifier plus ".jpg"

  Scenario: Sharing names a file by its identifier unless the policy selects the original name
    Given the owner has uploaded "Été à Nice.jpg"
    Then the default outgoing name should be the file identifier plus ".jpg"
    And the outgoing name under the original-name policy should be "Été à Nice.jpg"
