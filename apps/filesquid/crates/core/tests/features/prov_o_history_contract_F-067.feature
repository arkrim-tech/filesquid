# F-067: PROV-O history of every version and change
# SRS-F-067: PROV-O history of every version and change
# CORE-S-010: PROV-O history contract
Feature: PROV-O history contract types
  As a host crate
  I want shared types for operations, version messages, object versions and journal links
  So that every commit, record and API response names the same operation the same way

  Scenario: The version message of a commit round-trips
    When I format the operation message for operation "op-7" of kind "metadata.edit" with label "Before cleanup"
    Then the message should start with "filesquid.operation.v1 "
    And parsing it should give operation "op-7", kind "metadata.edit" and label "Before cleanup"

  Scenario Outline: Catalog organisation, publication and backfill have stable tokens
    When I format the operation message for operation "op-8" of kind "<kind>" with label "Release"
    Then parsing it should give operation "op-8", kind "<kind>" and label "Release"

    Examples:
      | kind                 |
      | catalog.organisation |
      | publish              |
      | backfill             |

  Scenario: A version committed before the rename is read as catalog organisation
    When I parse the version message "filesquid.operation.v1 {\"operation_id\":\"op-9\",\"kind\":\"catalog\"}"
    Then it should be read as kind "catalog.organisation" and written back as "catalog.organisation"

  Scenario: A message another tool wrote is not a FileSquid message
    When I parse the version message "Edit metadata.md"
    Then it should not be a FileSquid operation message

  Scenario: An unreadable FileSquid message is refused, not dropped
    When I parse the version message "filesquid.operation.v1 {\"kind\":\"import\"}"
    Then the message should be refused as unreadable

  Scenario: The installation agent is a UUID v4 URN
    When I parse the installation ID "6F1D2C3B-4A5E-4F60-8A7B-9C0D1E2F3A4B"
    Then its agent identifier should be "urn:uuid:6f1d2c3b-4a5e-4f60-8a7b-9c0d1e2f3a4b"

  Scenario: The PROV namespace is the W3C namespace IRI
    Then the PROV-O context should be "http://www.w3.org/ns/prov#" and the profile "filesquid-prov/1"

  # Source
  # - resources/prd.md F-067: PROV-O history of every version and change
  # - apps/filesquid/crates/core/resources/srs.md CORE-S-010
  # - resources/architecture/040-prov-o-history.md Decision 10
