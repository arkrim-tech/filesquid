# SRS-F-010: Export and portable backup
Feature: Export and portable backup
  As a library owner
  I want export and portable backup
  So that the product matches the SRS

  Scenario: Happy path for F-010
    Given the feature is enabled on a supported profile
    When I use it as specified in the PRD
    Then the MUST criteria for that feature should hold

  Scenario: Limitation is visible for F-010
    Given a platform cannot provide a capability
    When I view that capability
    Then I should see an explicit unsupported or waiting state

  Scenario: Failure does not corrupt originals for F-010
    Given an operation fails
    When I inspect the library
    Then originals and committed OCFL snapshots should remain

  Scenario: Owner downloads use the original file name
    Given an imported file shown as "Résumé final.pdf"
    When I download it
    Then the download should be named "Résumé final.pdf"
    And the response should carry a UTF-8 filename* parameter and a safe ASCII filename fallback

  Scenario: Two files with the same name in one export stay distinct
    Given two files shown as "photo.jpg" in different folders
    When I export both into one folder
    Then the exported names should be "photo.jpg" and "photo (1).jpg"

  Scenario: The FileId naming scheme remains an option
    When I export files and choose FileId names
    Then each file should be named by its FileId plus its extension

  # Source
  # - srs.md SRS-F-010
  # - prd.md F-010
