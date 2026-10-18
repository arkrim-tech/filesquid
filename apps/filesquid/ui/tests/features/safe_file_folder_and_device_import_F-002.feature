# SRS-F-002: Safe file, folder and device import
Feature: Safe file, folder and device import
  As a library owner
  I want safe file, folder and device import
  So that the product matches the SRS

  Scenario: Happy path for F-002
    Given the feature is enabled on a supported profile
    When I use it as specified in the PRD
    Then the MUST criteria for that feature should hold

  Scenario: Limitation is visible for F-002
    Given a platform cannot provide a capability
    When I view that capability
    Then I should see an explicit unsupported or waiting state

  Scenario: Failure does not corrupt originals for F-002
    Given an operation fails
    When I inspect the library
    Then originals and committed OCFL snapshots should remain

  Scenario: Every import commits a file record of source-system and host facts
    Given a source file on macOS with known birth, modified and accessed times, an owner, a mode, Finder tags and a quarantine attribute
    When I import it into a library
    Then the same OCFL version should contain the file and its file-record.md
    And the file record should hold the original name and path, every timestamp, the owner, group and mode, every extended attribute with the quarantine and Finder tags decoded, the source volume, the host OS version and the FileSquid version

  Scenario: Facts the source cannot provide are listed, never invented
    Given an upload whose client provides only a name, size, last-modified time and media type
    When the upload completes
    Then the file record should list every other fact as unavailable with a reason

  Scenario: Re-importing the same file appends to the file record
    Given a file that was imported once
    When the same bytes are imported again
    Then a new OCFL version should add a second acquisition entry
    And the first acquisition entry should be unchanged

  Scenario: Large extended attributes are bounded
    Given a source file with an extended attribute larger than 64 KiB
    When I import it
    Then the file record should keep that attribute's name, size and SHA-256 digest but not its value

  Scenario: Identity fields in the file record are scrubbed on export by default
    Given an imported file whose file record contains its original path, account name, host name and where-from URL
    When I export the file with the default outgoing policy
    Then the exported file record should contain none of those values
    And the file record in the library should be unchanged

  # Source
  # - srs.md SRS-F-002
  # - prd.md F-002
