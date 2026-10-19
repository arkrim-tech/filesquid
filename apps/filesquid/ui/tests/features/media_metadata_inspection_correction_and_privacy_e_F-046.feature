# SRS-F-046: Media metadata inspection, correction and privacy exports
Feature: Media metadata inspection, correction and privacy exports
  As a library owner
  I want Dublin Core descriptive metadata and EXIF technical metadata
  So that I can describe files with ISO 15836-2 terms and control what leaves with a share

  Scenario: Happy path for F-046
    Given the feature is enabled on a supported profile
    When I use it as specified in the PRD
    Then the MUST criteria for that feature should hold

  Scenario: Inspector lists populated keys only
    Given a file with dcterms:creator "George" and no other descriptive statements
    When I open the metadata inspector
    Then I should see dcterms:creator "George"
    And I should not see empty /terms/ properties as blank rows

  Scenario: Add a Dublin Core term
    Given I have selected a file
    When I add dcterms:creator with value "George"
    Then the inspector should show dcterms:creator "George"
    And the statement should bind to the file identity and digest

  Scenario: Schema.org overflow when no /terms/ match
    Given I want to record a certification that has no /terms/ property
    When I add descriptive metadata for that intent
    Then FileSquid should suggest the closest Schema.org term
    And after confirmation the inspector should show schema:certification with the entered value

  Scenario: Unknown dcterms local name is rejected
    Given I try to author dcterms:name
    When I save the statement
    Then the write should be rejected
    And I should be offered dcterms:title for a resource name or dcterms:creator for a person

  Scenario: Imported IPTC caption maps to dcterms:description
    Given a file whose IPTC caption is "Harbour at dusk"
    When extraction completes
    Then the owner-facing descriptive record should include dcterms:description "Harbour at dusk"
    And the raw IPTC observation should remain a separate extraction

  Scenario: Accrual terms are not authored on a file
    Given I try to author dcterms:accrualMethod on a file
    When I save the statement
    Then the write should be rejected

  Scenario: Namespace prefixes are visible
    Given populated exif:GPSLatitude, dcterms:creator and schema:certification values
    When I view the inspector
    Then each row should show its prefix

  Scenario: Limitation is visible for F-046
    Given a platform cannot provide a capability
    When I view that capability
    Then I should see an explicit unsupported or waiting state

  Scenario: Failure does not corrupt originals for F-046
    Given an operation fails
    When I inspect the library
    Then originals and committed OCFL snapshots should remain

  Scenario: Shared files default to FileId names
    Given a file shown as "Holiday 2023.jpg"
    When I share it with the default outgoing policy
    Then the shared file should be named by its FileId plus ".jpg"

  Scenario: The owner may keep the original name for shares
    Given a saved outgoing policy that selects the original name
    When I share a file shown as "Holiday 2023.jpg"
    Then the shared file should be named "Holiday 2023.jpg"

  # Source
  # - srs.md SRS-F-046
  # - prd.md F-046
  # - architecture/022-dublin-core-descriptive-metadata.md
