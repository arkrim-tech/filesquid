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

  Scenario: Every import commits an import record of source-system and host facts
    Given a source file on macOS with known birth, modified and accessed times captured before FileSquid reads it, an owner, a mode, Finder tags and a quarantine attribute
    When I import it into a library
    Then the same OCFL version should contain the file and its metadata record <file-uuid>.metadata.md with the import section
    And the import record should hold the original name and path, the file size in bytes as reported and as verified, every timestamp, the owner, group and mode, every extended attribute with the quarantine and Finder tags decoded, the source volume, the host OS version and the FileSquid version

  Scenario: A failure between the content and its import record leaves a retryable pending import
    Given an import whose content has been verified
    When the import fails before the import record is committed
    Then no OCFL version should hold the file without its import record
    And the import should be listed as pending and retryable

  Scenario: A size mismatch between source and stored bytes fails the import
    Given a source file whose reported size differs from the bytes copied into the library
    When I import it
    Then the import should fail verification and stay pending and retryable
    And no OCFL version should hold the file or its import record

  Scenario: A referenced import also gets an import record
    Given an explicit referenced import of a file that stays at its source location
    When the import completes
    Then the OCFL version that records the reference should also contain the import record
    And the import record should state that the bytes were referenced, not copied

  Scenario: Facts the source cannot provide are listed, never invented
    Given an upload whose client provides only a name, size, last-modified time and media type
    When the upload completes
    Then the import record should list every other fact as unavailable with a reason

  Scenario: Re-importing the same file appends to the import record
    Given a file that was imported once
    When the same bytes are imported again
    Then a new OCFL version should add a second acquisition entry
    And the first acquisition entry should be unchanged

  Scenario: Large extended attributes are bounded
    Given a source file with an extended attribute larger than 64 KiB
    When I import it
    Then the import record should keep that attribute's name, size and SHA-256 digest but not its value

  Scenario: Attribute inspection stops at the per-entry bounds and says so
    Given a source file with 300 extended attributes whose values total 3 MiB
    When I import it
    Then the acquisition entry should hold at most 256 attribute names and be marked truncated with the number enumerated
    And attribute values beyond the 1 MiB read budget should keep their name and size with the digest marked incomplete

  Scenario: Binary attribute names and values round-trip exactly
    Given a source file with an extended attribute whose name is not valid UTF-8 and another whose value is not valid UTF-8
    When I import it
    Then the import record should store that name and that value as base64 marked encoding base64
    And decoding each should give the original bytes

  Scenario: The Metadata panel never receives raw filesystem paths
    Given an imported file whose import record holds its original absolute path
    When the Metadata panel shows the import record
    Then it should show the original name, the path relative to the import root and the volume name
    And the API response should not contain the absolute path or the import root

  Scenario: Identity fields in the import record are scrubbed on export by default
    Given an imported file whose import record contains its original path, account name, host name and where-from URL
    When I export the file with the default outgoing policy
    Then the exported import record should contain none of those values
    And the import record in the library should be unchanged

  Scenario: An import stores the original under a universally unique name
    Given a source file named "Holiday 2023.jpg"
    When I import it into a library
    Then the original should be stored as its FileId followed by ".jpg"
    And the same directory should hold the file's metadata record named after the FileId with ".metadata.md"
    And the metadata record should hold the original file name "Holiday 2023.jpg", the extension, the format and MIME type and the import facts
    And no storage path should contain "Holiday 2023"

  Scenario: Every import is tracked and described without the c2pa module
    Given a build without the c2pa module
    When I import a JPEG with EXIF data
    Then the same OCFL version should hold the original, its metadata record with extracted dcterms statements and the import's PROV-O record
    And no C2PA record should be created

  Scenario: A rename changes only the display name
    Given an imported file shown as "Holiday 2023.jpg"
    When I rename it to "Beach.jpg"
    Then the file should be shown as "Beach.jpg"
    And its stored name and its recorded original file name should be unchanged

  Scenario: Original names are recoverable with ordinary tools
    Given a library with imported files
    When I close FileSquid and copy the storage root with ordinary file tools
    Then each stored original should have its metadata record beside it naming the original file name

  Scenario: A file name the extension rule rejects is stored without an extension
    Given a source file named "notes.tar~backup"
    When I import it into a library
    Then the original should be stored under its FileId alone
    And the metadata record should hold the original file name "notes.tar~backup"

  # Source
  # - srs.md SRS-F-002
  # - prd.md F-002
