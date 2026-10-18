# OCFL-S-002: Digest algorithm is SHA-256; OCFL 1.1 inventory, sidecar, NAMASTE and head types
# TC-004: Decision status and repository alignment (shared SHA-256 test vectors)
Feature: SHA-256 digests and OCFL 1.1 inventory types
  As a preservation programme that reads and writes OCFL objects
  I want the engine to hash with SHA-256 and to read inventories strictly
  So that every object it accepts follows OCFL 1.1 and every refusal names the rule

  # Happy path
  Scenario: SHA-256 is the engine's digest algorithm
    When I digest "abc" with the engine's digest algorithm
    Then the algorithm should be named "sha256"
    And the digest should be "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"

  # Happy path
  Scenario: A valid OCFL 1.1 inventory becomes typed data
    Given the example OCFL 1.1 inventory
    When I parse the inventory
    Then the head should be "v2"
    And the content directory should be "content"
    And the inventory should have 2 versions

  # Error cases: every rule refused with its OCFL 1.1 validation code
  Scenario Outline: An inventory that breaks an OCFL rule is refused with its code
    Given the example OCFL 1.1 inventory
    And the inventory is changed so that <change>
    When I parse the inventory
    Then the inventory should be refused with "<code>"

    Examples:
      | change                                            | code |
      | it has an unknown key                             | E102 |
      | it has no id                                      | E036 |
      | its digest algorithm is md5                       | E025 |
      | its head is not the highest version               | E040 |
      | its versions skip a number                        | E010 |
      | a version was created without a time zone         | E049 |
      | a content path ends with a slash                  | E100 |
      | a state digest differs in case from the manifest  | E050 |
      | two logical paths in a version conflict           | E095 |
      | a digest appears twice in the manifest            | E096 |

  # Edge case: untrusted input bounds
  Scenario: An oversized inventory is refused before it is parsed
    Given an inventory one byte larger than the parser's bound
    When I parse the inventory
    Then the inventory should be refused as too large

  # Sidecar
  Scenario: The inventory digest sidecar must match the inventory
    Given the example OCFL 1.1 inventory
    When I write its sha256 sidecar
    Then the sidecar should verify against the inventory
    And a sidecar for different bytes should be refused with "E060"

  # NAMASTE
  Scenario: An OCFL 1.1 object declares itself with NAMASTE
    When I make the OCFL 1.1 object declaration
    Then its file name should be "0=ocfl_object_1.1"
    And its contents should be "ocfl_object_1.1" and a newline

  # Source
  # - apps/filesquid/crates/ocfl/resources/srs.md OCFL-S-002
  # - resources/prd.md TC-004
  # - resources/architecture/009-cryptography-webcrypto-compatible-suite.md
  # - resources/architecture/010-ocfl-1-1-dual-profile.md
  # - OCFL 1.1 specification sections 3.2 to 3.7 and 4.2, and its validation codes
