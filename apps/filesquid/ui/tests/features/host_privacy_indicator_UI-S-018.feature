# F-057: Desktop and server hosts
# SRS-F-057: Desktop and server hosts (each host states its privacy boundary in the UI)
# F-059: Hosted encrypted backup and sync service
# UI-S-004: The UI states which host it is connected to and who can read files there
# UI-S-018: The host privacy indicator takes its facts only from the host descriptor
# API-S-020: Host descriptor
Feature: Host privacy indicator
  As a person using FileSquid
  I want the shell to tell me which host I am connected to and who can read my files there
  So that I never mistake a server someone else runs for my own device or for encrypted hosting

  # Happy path
  Scenario: The desktop host says only I can read my files
    Given the host descriptor says host type "desktop" and the hosted service is not enrolled
    When the shell loads
    Then the indicator should show the desktop statement from the label table
    And the hosted backup and sync area should show the not-enrolled state with neither backup nor sync on

  Scenario: A server names its operator
    Given the host descriptor says host type "server" with operator name "Garderen Family"
    When the shell loads
    Then the indicator should say the files are stored on a server run by "Garderen Family", who can read them
    And it should say that other users see only what they are permitted to
    And it should say that protecting the server's storage at rest is the operator's disk or bucket encryption

  Scenario: Enrolled backup and sync are shown apart from the host
    Given the host descriptor says host type "desktop" and the hosted service at "backup.filesquid.tech" has backup on and sync off
    When the shell loads
    Then the hosted service area should show backup on and sync off as two separate states with "backup.filesquid.tech"
    And it should state that files are encrypted on this device before upload
    And sync should never be labelled as backup

  # Edge cases
  Scenario: A server without an operator name is still honest
    Given the host descriptor says host type "server" with no operator name
    When the shell loads
    Then the indicator should use the unnamed-operator label and still say the operator can read the files

  Scenario: The indicator follows a change without a reload
    Given the shell shows the not-enrolled state
    When a "host.change" event reports that sync is on
    Then the indicator should show sync on without a page reload

  Scenario: The indicator rereads after missed events
    Given the shell shows the host indicator
    When the event channel delivers an "events.resync" hint
    Then the UI should read the host descriptor again

  # Error cases
  Scenario: The indicator never guesses the host
    Given the host descriptor cannot be read
    When the shell loads
    Then the indicator should show its error state
    And no host type or privacy statement should be shown

  # User interactions
  Scenario: The indicator is reachable by keyboard and screen reader
    Given the host descriptor says host type "desktop" and the hosted service is not enrolled
    When the user tabs to the host indicator
    Then it should have an accessible name from the label table and a visible focus outline
