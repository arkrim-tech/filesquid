# UI-S-012: The UI reaches FileSquid only through FileSquidClient (request, read, upload, subscribe),
# which selects the HTTP-plus-event-channel adapter on filesquid-server, the custom-scheme adapter
# on filesquid-desktop, and later the browser-host adapter.
Feature: One client for every FileSquid host
  As a library owner using the desktop app or a FileSquid server
  I want the same UI to reach whichever host serves it
  So that every distribution works the same on desktop and server

  # Happy path
  Scenario: The UI served by filesquid-server uses same-origin HTTP and the WebSocket channel
    Given the UI is served by filesquid-server
    When the shell reads API health and the Monitor subscribes to live events
    Then the requests should go to the page's own origin under "/api/v1/"
    And the events should arrive over the WebSocket at "/api/v1/events"

  Scenario: The UI inside the desktop app uses the custom URI scheme and Tauri events
    Given the UI runs inside the filesquid-desktop webview on Windows
    When the shell reads API health and the Monitor subscribes to live events
    Then the requests should go to "http://filesquid.localhost/api/v1/"
    And no request should go to the page's own origin
    And the events should arrive as the Tauri event "filesquid-event"
    And no WebSocket should be opened

  Scenario Outline: The desktop adapter picks the scheme form of the platform
    Given the UI runs inside the filesquid-desktop webview on <platform>
    Then the API base URL should be "<base>"

    Examples:
      | platform | base                       |
      | macOS    | filesquid://localhost      |
      | Linux    | filesquid://localhost      |
      | Windows  | http://filesquid.localhost |
      | Android  | http://filesquid.localhost |

  # Commands
  Scenario: A committed command is the only completion evidence
    Given a command envelope with an operation id, the revision last read and a scope
    When the client sends the command and the API answers "committed"
    Then the view may show the change as done
    But a "pending" answer should not be shown as done

  Scenario: A stale revision is reported as a conflict
    Given a command envelope whose expected revision is out of date
    When the API answers HTTP 409 conflict
    Then the client should return a "conflict" outcome for the same operation id

  Scenario: A retry reuses the operation id
    Given a command was sent and its answer was lost
    When the view retries the command
    Then the retry should carry the same operation id

  # Edge cases
  Scenario: A ranged read returns part of a file
    Given a file of 100 bytes
    When the view reads bytes 0 to 9
    Then the request should carry "Range: bytes=0-9"
    And the result should be partial with content range 0 to 9 of 100

  Scenario: An invalid range is refused before anything is sent
    When the view asks for bytes 5 to 1
    Then the client should refuse the range without sending a request

  # Error cases
  Scenario: An answer for another operation is not accepted as completion
    Given the client sent a command with operation id "op-1"
    When the API answers with an outcome for "op-9"
    Then the client should reject the answer

  Scenario: A refused command keeps its denial reason
    Given no token has been entered on filesquid-server
    When the view sends a command
    Then the client should raise an error with reason "missing_credentials"
