# API-S-028: History object — every library has exactly one PROV-O history object (ADR 040
# Decision 5) at a fixed identifier and storage-root path derived from the library ID alone,
# created with the library; a library opened without one is given one, journaled once, before its
# first export or ZIP download. Version message — a caller's version message is the version label
# (1 to 512 bytes, not blank, no control characters).
# API-S-011: ZIP download — streamed from ranged reads within 1,000 members and 10 GiB, with a
# manifest.txt naming omitted members, never held in memory; the owner's own ZIP applies no
# per-member location or identity scrubbing and the dialog says so; members are the stored contents
# under their download names, never the import records; its PROV-O activity goes to the history
# object and it is journaled once with opaque IDs only.
# Owner decision 2026-10-31, verbatim: "accept recommendations" (decisions 9, 10 and 15 (a)).
# Scenarios only; Task API-F2.22 adds their step definitions (unmatched steps are reported as skipped).
Feature: Library history object and streamed ZIP download
  As a library owner
  I want my downloads streamed and recorded in my library's history
  So that a large download never exhausts memory and my history explains every output

  # Happy path
  Scenario: A new library has its history object
    When zip-history caller creates the library "Archive"
    Then zip-history library "Archive" should hold exactly one history object at its fixed identifier and path

  Scenario: A ZIP download streams its members and records its activity
    Given zip-history library with 3 files in collection "Summer"
    When zip-history caller downloads those 3 files as one ZIP
    Then zip-history archive should hold the 3 stored contents under their download names and a manifest.txt
    And zip-history archive should never be held whole in memory
    And zip-history history object should gain one provenance-only version for the download
    And zip-history download should be journaled once with opaque IDs only

  # Edge cases
  Scenario: A library opened without a history object gets one before its first download
    Given zip-history library on disk with no history object
    When zip-history caller opens it and downloads one file as a ZIP
    Then zip-history library should hold exactly one history object, created and journaled once

  Scenario: The owner's own ZIP keeps the metadata in its members
    Given zip-history file "harbour.jpg" whose stored content holds a location
    When zip-history caller downloads it as a ZIP
    Then zip-history member "harbour.jpg" should equal its stored content byte for byte
    And zip-history archive should hold no import record

  Scenario: A member that cannot be read is named in the manifest
    Given zip-history library with one unreadable file among 3
    When zip-history caller downloads those 3 files as one ZIP
    Then zip-history manifest.txt should name the unreadable file as omitted

  # Error cases
  Scenario Outline: A ZIP over its limits is refused before anything is written
    When zip-history caller asks for a ZIP of <members> members totalling <bytes>
    Then zip-history answer should be refused as invalid_request and nothing journaled as a download

    Examples:
      | members | bytes   |
      | 1001    | 1 MiB   |
      | 2       | 11 GiB  |

  Scenario Outline: A version message is the version label within its bounds
    When zip-history caller commits an edit with the version message <message>
    Then zip-history answer should be <outcome>

    Examples:
      | message                        | outcome                    |
      | "Sent to printer"              | committed with that label  |
      | a blank message                | refused as invalid_request |
      | a message of 513 bytes         | refused as invalid_request |
      | a message with a control byte  | refused as invalid_request |
