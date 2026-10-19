# F-067: PROV-O history of every version and change (server accounts as agents)
# SRS-F-067: Installation UUID and agents; ADR 040 Decision 4 (planned approach, finalised in Task FS-3.10)
# Scenarios only; step definitions arrive with Task FS-3.10 (unbound steps are skipped by the harness).
Feature: Server accounts are PROV-O agents acting on behalf of the installation
  As a server administrator
  I want each account's changes attributed to that account without exposing who the person is
  So that a multi-user library keeps a history of who acted, by opaque identifier only

  Background:
    Given a filesquid-server installation with a known installation UUID
    And the account "ana" with write access to a library

  # Happy path
  Scenario: A server account is its own agent acting on behalf of the installation
    When "ana" renames a file
    Then the account UUID of "ana" should be a UUID version 4
    And the activity should be prov:wasAssociatedWith the agent "urn:uuid:" followed by the account UUID of "ana"
    And that account agent should prov:actedOnBehalfOf the server's installation agent
    And the activity should be prov:wasAssociatedWith the server's FileSquid software agent and its version
    And the record should contain no prov:Person, user name, e-mail address or display name

  # Source
  # - resources/srs.md SRS-F-067
  # - resources/prd.md F-067
  # - resources/architecture/040-prov-o-history.md Decision 4
