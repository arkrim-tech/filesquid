# F-057: Desktop and server hosts
# SRS-F-057: Desktop and server hosts (the API tells the UI which host it is connected to)
# F-059: Hosted encrypted backup and sync service
# SRS-F-059: Hosted encrypted backup and sync service (ciphertext only; backup and sync are distinct)
# API-S-020: Host descriptor: host type, operator name and hosted backup and sync state, with a change event
# API-S-003: One bounded event channel; a client rereads state after a resync hint
# API-S-015: The liveness probe is the only public route
#
# Step definitions arrive with Task API-F1.10; until then these scenarios are reported as skipped.
Feature: Host descriptor
  As a person using any FileSquid distribution
  I want the API to tell my UI which host it is connected to, who operates it and whether encrypted backup and sync are on
  So that the privacy statement I see is true and never guessed

  # Happy path
  Scenario: The desktop host describes itself to the owner's webview
    Given a FileSquid desktop host that describes itself to the API
    When the owner's webview reads the host descriptor with the per-launch token
    Then the descriptor should say host type "desktop" with no operator name
    And the hosted service should be reported as not enrolled

  Scenario: A server describes itself with its operator's name to any signed-in caller
    Given a FileSquid server host whose operator name is set to "Garderen Family"
    When a signed-in member who is not an administrator reads the host descriptor
    Then the descriptor should say host type "server" with operator name "Garderen Family"

  Scenario: Backup and sync are reported separately once enrolled
    Given a FileSquid desktop host enrolled with the hosted service at "backup.filesquid.tech" with backup on and sync off
    When the owner's webview reads the host descriptor with the per-launch token
    Then the hosted service should be enrolled at "backup.filesquid.tech"
    And backup should be reported on and sync off, as two separate values

  # Edge cases
  Scenario: The host type never follows the request
    Given a FileSquid desktop host that describes itself to the API
    When a caller reads the host descriptor with headers that claim a server origin and a browser user agent
    Then the descriptor should still say host type "desktop"

  Scenario: A server without an operator name starts and says so
    Given a FileSquid server host started without FILESQUID_OPERATOR_NAME
    When a signed-in member who is not an administrator reads the host descriptor
    Then the descriptor should say host type "server" with no operator name
    And the activity journal should hold one "server.operator_unnamed" warning that names the variable

  Scenario Outline: An unusable operator name stops the server
    Given FILESQUID_OPERATOR_NAME is set to <value>
    When filesquid-server starts
    Then it should refuse to start with an error that names FILESQUID_OPERATOR_NAME
    And the error should not contain the value

    Examples:
      | value                              |
      | a name of 257 characters           |
      | a name with a control character    |

  Scenario: A descriptor change reaches every subscriber
    Given a FileSquid desktop host that describes itself to the API
    And an owner's webview and a second authenticated client subscribed to the event channel
    When the host enrols with the hosted service and turns sync on
    Then each subscriber should receive one "host.change" event carrying the whole new descriptor

  Scenario: A client that missed events reads the descriptor again
    Given a subscriber that received an "events.resync" hint
    When it recovers its state
    Then it should read the host descriptor through the request route

  # Error cases
  Scenario: A caller without a credential is refused
    Given a FileSquid desktop host that describes itself to the API
    When a client reads the host descriptor without a credential
    Then it should be refused as "unauthorized" because of "missing_credentials"
    And the activity journal should record one "api.auth.denied" entry

  Scenario: The descriptor and its event carry nothing private
    Given a FileSquid desktop host enrolled with the hosted service at "backup.filesquid.tech" with backup on and sync on
    When the descriptor and a "host.change" payload are inspected
    Then neither should contain a token, content handle, key, account or device identifier, e-mail address, quota, size, timestamp, path or file name
