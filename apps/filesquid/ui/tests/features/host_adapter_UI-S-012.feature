# UI-S-012: The UI reaches FileSquid only through FileSquidClient (request, read, command, upload, subscribe),
# which selects the HTTP-plus-event-channel adapter on filesquid-server, the custom-scheme adapter
# on filesquid-desktop, and later the browser-host adapter. Every command and upload carries a
# CommandEnvelope; a 2xx answer resolves to a CommandOutcome for the same operation id (error arrives
# only as a 2xx outcome body), HTTP 409 resolves to conflict, any other non-2xx answer rejects with
# ApiError, and only committed is completion evidence.
Feature: One client for every FileSquid host
  As a library owner using the desktop app or a FileSquid server
  I want the same UI to reach whichever host serves it
  So that every distribution works the same on desktop and server

  # Happy path
  Scenario: The UI served by filesquid-server uses same-origin HTTP and the WebSocket channel
    Given the UI is served by filesquid-server
    And the client has a valid server token
    When the shell reads API health and the Monitor subscribes to live events
    Then the requests should go to the page's own origin under "/api/v1/"
    And the events should arrive over the WebSocket at "/api/v1/events"

  Scenario: The UI inside the desktop app uses the custom URI scheme and Tauri events
    Given the UI runs inside the filesquid-desktop webview on Windows
    And the desktop host has given its own page a per-launch token
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
      | iOS      | filesquid://localhost      |
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

  Scenario: An upload carries its envelope and returns an outcome
    Given a command envelope with an operation id, the revision last read and a scope
    When the view uploads a file
    Then the request should be multipart form data with an "envelope" part and a "file" part
    And the client should return the API's outcome for the same operation id

  Scenario: A file write is a command whose committed outcome carries what it made (API-F2.13)
    Given the file browser shows an object at its head version
    When the view moves a file in it
    Then the request should carry an envelope expecting that head version and the move as its input
    And the view should receive the version the committed outcome names as its result

  Scenario: A file upload sends its facts as an input part
    Given a command envelope expecting revision "0" for a new file
    When the view uploads a file into a library
    Then the request should be a multipart PUT with "envelope", "input" and "file" parts

  # Edge cases
  Scenario: A file write against an out-of-date head changes nothing
    Given the object changed after the file browser listed it
    When the view removes a file with the head it listed
    Then the API should answer HTTP 409 with a conflict outcome
    And the view should show the conflict and change nothing

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
