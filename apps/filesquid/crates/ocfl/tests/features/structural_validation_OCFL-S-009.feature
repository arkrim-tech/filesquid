# OCFL-S-009: Validator conformance; errors and warnings use the OCFL 1.1 validation codes (E###, W###)
# OCFL-S-004: Only the root inventory commits a version; a version-directory inventory and sidecar alone are not a commit
Feature: Structural validation of OCFL 1.1 objects
  As a preservation programme using filesquid-ocfl
  I want every OCFL 1.1 rule an object breaks reported with its validation code
  So that I can tell a valid object from an invalid one and know why

  Background:
    Given a valid OCFL 1.1 object "obj" with three versions in an in-memory storage backend

  # Happy path
  Scenario: A valid object validates with zero errors and zero warnings
    When I validate the object
    Then the object should be valid
    And no warning should be reported

  # Edge case: a warning does not make an object invalid
  Scenario: A missing version inventory is a warning, not an error
    Given the file "v1/inventory.json" is removed
    And the file "v1/inventory.json.sha512" is removed
    When I validate the object
    Then the object should be valid
    And warning "W010" should be reported

  # Edge case: only the root inventory commits a version
  Scenario: A version directory that the root inventory does not list is not a commit
    Given a version directory "v4" with its own inventory and sidecar
    When I validate the object
    Then the object should be invalid
    And error "E046" should be reported

  # Edge case: an unregistered extension is a warning
  Scenario: An unregistered object extension is a warning
    Given the file "extensions/unregistered/config.json" is added
    When I validate the object
    Then the object should be valid
    And warning "W013" should be reported

  # Error cases
  Scenario Outline: A broken rule is reported with its OCFL 1.1 validation code
    Given the file "<file>" is <change>
    When I validate the object
    Then the object should be invalid
    And error "<code>" should be reported

    Examples:
      | file                  | change   | code |
      | extra.txt             | added    | E001 |
      | 0=ocfl_object_1.1     | removed  | E003 |
      | inventory.json        | removed  | E063 |
      | inventory.json.sha512 | removed  | E058 |
      | v1/extra.txt          | added    | E015 |
      | v1/content/extra.txt  | added    | E023 |
      | v1/content/b.txt      | removed  | E092 |
      | v1/content/a.txt      | replaced | E092 |
      | extensions/extra_file | added    | E067 |

  # Source
  # - resources/prd.md F-051, F-042, NFR-001
  # - apps/filesquid/crates/ocfl/resources/srs.md OCFL-S-009, OCFL-S-004
  # - resources/architecture/016-rust-ocfl-library.md ("Published head (readers)")
  # - https://ocfl.io/1.1/spec/validation-codes.html
