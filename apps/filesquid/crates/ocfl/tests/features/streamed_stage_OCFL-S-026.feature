# OCFL-S-026: Streamed single-file stage. The change model MUST let a caller stage one logical file from a file of any storage backend without reading the whole file into memory: bounded chunks of at most MAX_CHUNK_BYTES (8 MiB), hashed as they are copied, the sha256 digest answered; manifest bytes are not copied; a held blob is re-hashed before reuse and replaced when it does not match; the source is read at most twice and refused as SourceChanged when the two reads differ; the commit keeps the write order of OCFL-S-018.
# F-051: OCFL module
# NFR-003: Bounded processing and migration endurance
Feature: Streamed single-file stage
  As a host importing large files into an OCFL object
  I want to stage one file from any backend without reading it into memory
  So that memory use does not grow with the file and the staged bytes always match their digest

  Background:
    Given an object committed at "v1" with "a.txt", "b.txt" and "c.txt" through the change model

  # Happy path
  Scenario Outline: A file larger than one chunk is staged in bounded chunks from the <backend> backend
    Given a source file of three chunks on the <backend> backend
    When writer "writer-a" stages it as "big.raw" with stage_file
    Then stage_file should answer the sha256 digest of the source file
    And the source should have been read in chunks of at most 8 MiB, once to hash it and once to copy it
    When writer "writer-a" commits the streamed file
    Then the committed "big.raw" should hold the source file's bytes
    And the commit should keep the OCFL-S-018 write order for "big.raw"

    Examples:
      | backend    |
      | memory     |
      | filesystem |

  # Edge case
  Scenario: Bytes the object already holds are not copied
    Given a source file holding the bytes of "a.txt" on the memory backend
    When writer "writer-a" stages it as "copy-of-a.txt" with stage_file
    Then stage_file should answer the sha256 digest of the source file
    And the source should have been read once, to hash it
    And no blob should have been copied into the staging area

  # Edge case
  Scenario: A corrupt or stale staged blob is replaced with the source's bytes
    Given a source file of three chunks on the memory backend
    And the staging area already holds other bytes under the source file's digest
    When writer "writer-a" stages it as "big.raw" with stage_file
    Then stage_file should answer the sha256 digest of the source file
    And the staged blob should hold the source file's bytes

  # Error case
  Scenario: An invalid logical path is refused before the source is read
    Given a source file of three chunks on the memory backend
    When writer "writer-a" stages it as "../escape" with stage_file
    Then stage_file should be refused as an invalid logical path
    And the source should not have been read

  # Error case
  Scenario: A source that changes between its two reads is refused
    Given a source file of three chunks on the memory backend that changes after it is hashed
    When writer "writer-a" stages it as "big.raw" with stage_file
    Then stage_file should be refused because the source changed
    And no staged blob should be left under either digest

  # Source
  # - apps/filesquid/crates/ocfl/resources/srs.md OCFL-S-026, OCFL-S-014, OCFL-S-018
  # - resources/prd.md F-051, NFR-003
