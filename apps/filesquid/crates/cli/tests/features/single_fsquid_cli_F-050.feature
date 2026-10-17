# F-050: FileSquid platform and distributions
# SRS-F-050: FileSquid platform and distributions (single `fsquid` CLI, C-057)
Feature: One command-line tool named fsquid
  As a scripter or preservation programme
  I want every FileSquid command to start with fsquid
  So that I learn one tool instead of one per module

  Scenario: A command that only one module defines runs without the module name
    When I run fsquid with "get dcterms:title"
    Then the metadata module should handle it

  Scenario: Any command also runs with its module name
    When I run fsquid with "metadata get dcterms:title"
    Then the metadata module should handle it

  Scenario: validate always needs the module name
    When I run fsquid with "validate dcterms:title"
    Then fsquid should refuse and suggest "fsquid metadata validate"

  Scenario: validate runs with the module name
    When I run fsquid with "metadata validate dcterms:title"
    Then the command should succeed

  Scenario: A module name alone with help lists its commands
    When I run fsquid with "metadata help"
    Then the metadata module should handle it
    And the command should succeed
    And the routed help should list extract, get, set, export, import, and validate

  Scenario: An unknown command is reported
    When I run fsquid with "mods"
    Then fsquid should report an unknown command

  Scenario: fsquid is the only CLI binary
    When I list the CLI binaries the workspace builds
    Then the only CLI binary should be fsquid
