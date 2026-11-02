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

  # Upload dialog, review and Jobs panel (Task UI-1.2; UI-S-002 Destination, UI-S-028, UI-S-029, API-S-036, API-S-037)
  Scenario: The upload dialog needs a destination collection before anything is sent
    Given the upload dialog is open and no collection or folder is chosen
    Then the start control should be disabled and the dialog should say "Choose a destination first."
    And no request should be sent to the host
    When I choose a folder inside a collection in the destination tree
    Then the start control should be enabled and read "Choose files…" or "Choose folder…" for the chosen source

  Scenario: Only collections and folders inside them are offered as a destination
    Given the destination tree lists libraries, collections, folders, a virtual collection and Recently deleted
    Then each library row should say it is not a destination
    And a virtual collection and Recently deleted should not be offered

  Scenario: The plan request carries the library, the collection, the folder and the source
    Given I have chosen the folder "Site photos" of the collection "Harbour project"
    When I ask to choose files
    Then the host should receive one import request with that library, collection ID, folder and the source "files"
    And the request should carry no file path or file name

  Scenario: Planning shows no percentage
    Given the host is planning what I picked
    Then the dialog should say it is waiting for my choice and checking it
    And it should show no percentage

  Scenario: The review is shown before the job starts and Cancel discards the plan
    Given the host answered a plan of 214 files found, 196 new, 18 already in the library and 3.1 GB to copy
    Then the review should show those counts, each planned file as New or "Already uploaded · skipped", the destination, the space after the upload and the integrity note
    And no job should have been started
    When I choose Cancel
    Then the plan should be discarded and no job should exist

  Scenario: Upload starts the job and the Jobs panel follows it
    Given I am reviewing a plan
    When I choose "Upload 196 files"
    Then the job should be started from that plan only
    And the Jobs panel should open on the job and the footer should read "Uploading · {done} of {total}"

  Scenario: Each file shows its status and error code
    Given a running upload job whose files are copied, copying at 62 percent, queued and failed with "source_changed"
    Then the Jobs panel should list "Copied", "Copying · 62%", "Queued" and "Failed · source_changed" with each file's destination folder

  Scenario: A failed file is retried on its own, then every failed file is retried
    Given an upload job with failed files
    When I choose Retry on one file
    Then only that file should be put back to queued
    When I choose "Retry failed"
    Then every failed file should be put back to queued

  Scenario: Only the controls the job accepts are offered
    Given a paused job, a finished job and a running job
    Then the paused job should offer Resume and Cancel and no Retry
    And the finished job should offer no control
    And the running job should offer Pause and Cancel

  Scenario: A job that ended with a failed file waits for the owner
    Given an upload job that is pending with every file ended and one failed
    Then the Jobs panel should show it as needing attention with "Retry failed", Retry and Cancel
    And it should show no progress bar and no motion

  Scenario: Refusals are honest errors with a Dismiss control
    Given the host refused the plan request because the chosen collection is gone
    Then the dialog should show "The collection you chose is no longer in this library. Choose another and try again."
    And a file that failed with "path_suffix_exhausted" should show the first and last numbered name and say nothing was changed
    And each error card should offer Dismiss

  Scenario: A host that cannot name the chosen collection refuses before sending
    Given the host cannot say which collection the chosen node is
    When I ask to choose files
    Then the dialog should say it cannot name the collection and nothing should be sent

  Scenario: The default UI says Upload, not Import
    Given the default label table is active
    Then the dialog, the New menu and the Jobs panel should say Upload
    And the word Import should not appear in them

  # Source
  # - srs.md SRS-F-002
  # - prd.md F-002
  # - apps/filesquid/ui/resources/srs.md UI-S-002, UI-S-028, UI-S-029
  # - apps/filesquid/crates/api/resources/srs.md API-S-036, API-S-037
