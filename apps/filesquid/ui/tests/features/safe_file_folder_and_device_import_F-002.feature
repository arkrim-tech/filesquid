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
    Given a source file on macOS with known birth, modified and accessed times captured before FileSquid reads it, an owner, a mode, Finder tags and a quarantine attribute
    When I import it into a library
    Then the same OCFL version should contain the file and its file-record.md
    And the file record should hold the original name and path, every timestamp, the owner, group and mode, every extended attribute with the quarantine and Finder tags decoded, the source volume, the host OS version and the FileSquid version

  Scenario: A failure between the content and its file record leaves a retryable pending import
    Given an import whose content has been verified
    When the import fails before the file record is committed
    Then no OCFL version should hold the file without its file record
    And the import should be listed as pending and retryable

  Scenario: A referenced import also gets a file record
    Given an explicit referenced import of a file that stays at its source location
    When the import completes
    Then the OCFL version that records the reference should also contain the file record
    And the file record should state that the bytes were referenced, not copied

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

  Scenario: Attribute inspection stops at the per-entry bounds and says so
    Given a source file with 300 extended attributes whose values total 3 MiB
    When I import it
    Then the acquisition entry should hold at most 256 attribute names and be marked truncated with the number enumerated
    And attribute values beyond the 1 MiB read budget should keep their name and size with the digest marked incomplete

  Scenario: Binary attribute names and values round-trip exactly
    Given a source file with an extended attribute whose name is not valid UTF-8 and another whose value is not valid UTF-8
    When I import it
    Then the file record should store that name and that value as base64 marked encoding base64
    And decoding each should give the original bytes

  Scenario: The Metadata panel never receives raw filesystem paths
    Given an imported file whose file record holds its original absolute path
    When the Metadata panel shows the file record
    Then it should show the original name, the path relative to the import root and the volume name
    And the API response should not contain the absolute path or the import root

  Scenario: Identity fields in the file record are scrubbed on export by default
    Given an imported file whose file record contains its original path, account name, host name and where-from URL
    When I export the file with the default outgoing policy
    Then the exported file record should contain none of those values
    And the file record in the library should be unchanged

  # Source
  # - srs.md SRS-F-002
  # - prd.md F-002
