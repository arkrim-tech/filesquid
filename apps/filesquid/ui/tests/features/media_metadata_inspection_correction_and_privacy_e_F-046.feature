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

  # Task UI-F2.4: the Description tab of the file panel (UI-S-022, UI-S-031 mockups 28, 29
  # and 25; API-S-035; MD-S-011): the dcterms and schema statements as rows keyed by their
  # CURIE, extracted versus owner-asserted values, Add property, edit and remove
  Scenario: The Description tab lists descriptive statements keyed by their CURIE
    Given a file with populated dcterms:, schema: and exif: statements
    When I open the file's Description tab
    Then I should see one row per dcterms: and schema: statement, keyed by its CURIE with the prefix
    And the exif: statements should be left to the File info tab's technical rows
    And the count should read "Dublin Core and Schema.org" with the number of rows

  Scenario: Owner values and extracted values are told apart
    Given a file whose dcterms:title the owner asserted and whose dcterms:created was extracted from the file
    When I open the file's Description tab
    Then the owner value should carry the chip "You" and offer edit and remove
    And the extracted value should carry the chip "File" and offer copy only

  Scenario: Adding a property offers Dublin Core first, then Schema.org, then registered namespaces
    Given the library has registered an extension namespace and declared a custom namespace
    When I choose Add property and type "righ"
    Then I should see the Dublin Core terms that match, each with its definition
    And then the Schema.org suggestion, marked as added only after I confirm
    And then the registered namespaces that accept the name
    And a key in a custom namespace should be offered only because a custom namespace is declared

  Scenario: Adding a property commits one version and lists the new row
    Given I have chosen dcterms:rightsHolder in the Add property dialog
    When I enter a value and confirm
    Then one statement should be set in one new version of the file's record
    And the Description tab should list the new row and announce the version

  Scenario: Editing or removing an owner value commits one version
    Given an owner-asserted dcterms:title
    When I edit its value, or remove it
    Then one statement should be replaced or removed in one new version of the file's record
    And the Description tab should list again from the new version

  Scenario: A refused statement leaves the dialog open with the reason
    Given I confirm a value for a property the register does not know
    When the API refuses the statement
    Then the dialog should stay open and show why
    And nothing should be changed

  Scenario: The Description tab of a file without descriptive statements is empty
    Given a file whose record holds no descriptive statement
    When I open its Description tab
    Then I should see "No description yet" with a hint and Add property
    And the count should read "0 properties"

  Scenario: The Description tab shows placeholder rows while statements load
    Given a file whose statements have not yet been answered
    When I open its Description tab
    Then I should see placeholder rows and "Loading description…"
    And the filter should be disabled until the rows arrive

  Scenario: A filter word that matches no property offers Clear filter
    Given the Description tab lists a file's statements
    When I filter with a word that matches no property or value
    Then I should see that no properties match the word
    And Clear filter should show every row again

  Scenario: A failed statements read shows the reason and Try again
    Given the statements read fails
    When I open the file's Description tab
    Then I should see "Could not load the description" with the reason, "Nothing was changed." and the code
    And Try again should read the statements again

  # Source
  # - srs.md SRS-F-046
  # - prd.md F-046
  # - architecture/022-dublin-core-descriptive-metadata.md
