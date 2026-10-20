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

  Scenario: The header shows a compact badge and the full statement in its popover and in Settings
    Given the host descriptor says host type "desktop" and the hosted service is not enrolled
    When the shell loads
    Then the header should show the host indicator as an icon and a short host label from the active label table without the full statement
    And opening the indicator's popover should show the desktop statement from the label table
    And the Privacy section of the Settings page should show the same statement

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

  Scenario: The browser host says only I can read my files, in this browser
    Given the host descriptor says host type "browser" and the hosted service at "backup.filesquid.tech" has backup on and sync on
    When the shell loads
    Then the indicator should show the browser-host statement that files are decrypted only in this browser
    And the hosted service area should show backup on and sync on as two separate states with "backup.filesquid.tech"
    And it should state that the service stores only ciphertext and never has the keys

  # Edge cases
  Scenario: The indicator shows a loading state until the descriptor arrives
    Given the host descriptor answer is delayed
    When the shell loads
    Then the indicator should show its loading state
    And no host type or privacy statement should be shown until the descriptor arrives

  Scenario: A server without an operator name is still honest
    Given the host descriptor says host type "server" with no operator name
    When the shell loads
    Then the indicator should use the unnamed-operator label and still say the operator can read the files

  Scenario: The indicator follows a change without a reload
    Given the shell shows the not-enrolled state
    When a "host.change" event reports that sync is on
    Then the indicator should show sync on without a page reload

  Scenario Outline: The indicator rereads after missed events
    Given the shell shows the host indicator
    When <interruption>
    Then the UI should read the host descriptor again
    And the indicator should show the descriptor it read

    Examples:
      | interruption                                         |
      | the event channel delivers an "events.resync" hint   |
      | the event channel disconnects and reconnects         |

  # Error cases
  Scenario: The indicator never guesses the host
    Given the host descriptor cannot be read
    When the shell loads
    Then the indicator should show its error state
    And no host type or privacy statement should be shown

  Scenario: A descriptor that breaks the contract is not shown
    Given the host descriptor answer has an unknown host type
    When the shell loads
    Then the indicator should show its error state
    And no host type or privacy statement should be shown

  Scenario: Retry reads the descriptor again
    Given the indicator shows its error state
    When the user chooses Retry
    Then the UI should read the host descriptor again
    And the indicator should show the descriptor it read

  # User interactions
  Scenario: The indicator is reachable by keyboard and screen reader
    Given the host descriptor says host type "desktop" and the hosted service is not enrolled
    When the user tabs to the host indicator
    Then it should have an accessible name from the label table and a visible focus outline

  # Compact badge and popover (owner decision 2026-10-19, option 1a)
  Scenario: The header shows a compact badge, not the statement
    Given the host descriptor says host type "desktop" and the hosted service is not enrolled
    When the shell loads
    Then the header should show the host badge "This device" with an icon
    And the privacy statement should not be permanent header text

  Scenario Outline: The popover opens by click or keyboard and carries the full statement
    Given the host descriptor says host type "desktop" and the hosted service is not enrolled
    When the user <opens> the host badge
    Then a popover named "Where your files are" should open with focus in it
    And it should say "Your files stay on this device. Only you can read them."
    And it should show the not-enrolled state of the backup and sync service

    Examples:
      | opens                 |
      | clicks                |
      | presses Enter on      |
      | presses Space on      |

  Scenario: Escape or a click outside closes the popover
    Given the host popover is open
    When the user presses Escape
    Then the popover should close and focus should return to the host badge
    When the user opens the popover again and clicks outside it
    Then the popover should close

  Scenario Outline: The badge names the host type in short
    Given the host descriptor says host type "<host type>"
    When the shell loads
    Then the host badge should read "<badge>"

    Examples:
      | host type | badge        |
      | desktop   | This device  |
      | server    | Server       |
      | browser   | This browser |

  Scenario Outline: The badge never guesses the host
    Given the host descriptor read fails with <failure>
    When the shell loads
    Then the host badge should read "Host unknown"
    And its popover should show the <state> state

    Examples:
      | failure                          | state                       |
      | HTTP 500 while online            | error state with Retry      |
      | HTTP 503 "unavailable"           | unavailable state with Retry |
      | a network error while offline    | offline state with Retry    |
      | HTTP 401 "missing_credentials"   | permission-denied state     |

  Scenario: The popover links to the Settings page's Privacy section
    Given the host popover is open
    When the user follows "Privacy settings"
    Then the Settings page should open with its Privacy section
    And the Privacy section should show the full statement for the connected host
    And the popover should close
