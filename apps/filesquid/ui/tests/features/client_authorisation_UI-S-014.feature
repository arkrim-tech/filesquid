# UI-S-014: FileSquidClient authenticates to the deny-by-default FileSquid API
Feature: Client authorisation against the deny-by-default API
  As a library owner or server administrator
  I want the UI to present my credential on every call and tell me why a call was refused
  So that the API stays deny-by-default without hiding the reason from me

  Scenario: The desktop app uses its per-launch token
    Given the desktop host has given its own page a per-launch token
    When the shell reads API health
    Then the request should carry that token as a bearer credential
    And the API status should read ok

  Scenario: The event channel offers the token as a subprotocol
    Given the client knows a token
    When the Monitor subscribes to live events in a browser
    Then the WebSocket should offer "filesquid.events.v1" first
    And it should offer "filesquid.bearer.<token>" second

  Scenario: A server page without a token names the missing credentials
    Given the UI is served by filesquid-server
    And no token has been entered
    When the shell reads API health
    Then the shell should show the "missing credentials" label from the active label table

  Scenario: The administrator signs in on the Monitor
    Given the Monitor shows its token form because credentials are missing
    When the administrator enters the server's administrator token
    Then the Monitor should show live health, processes and the journal
    And the shell should reload its API status as ok

  Scenario: A wrong token is named as invalid credentials
    Given the Monitor shows its token form
    When the administrator enters a token the server does not accept
    Then the Monitor should show the "invalid credentials" label as an alert
    And the token form should stay available

  Scenario: A known caller without permission is told so
    Given the API refuses a call with reason "not_permitted"
    When the UI shows the failure
    Then it should show the "not permitted" label, distinct from invalid credentials

  Scenario: The token never leaves memory
    Given the administrator has signed in
    Then the token should not be in local storage, session storage, the URL or the page markup
    And no error message should contain the token
    And reloading the page should ask for the token again

  Scenario: An unusable token is refused before sending
    Given the configured token is empty or contains characters a header or subprotocol cannot carry
    When the client makes a call
    Then no request should be sent
    And the call should fail with status 0 and reason "invalid_credentials"
    And the error should not echo the token

  # Content handles for media elements (UI-S-014, API-S-018; built with UI-F2.2)
  Scenario: A native preview loads through a content handle, never the token
    Given the client knows a token
    When the file browser shows a native video preview of "holiday.mp4"
    Then the video source should be a content handle URL on the active host's API base
    And no URL, history entry, storage entry or page markup should contain the token
    And the content handle URL should not be in the page address, history, storage or any link that opens another window

  Scenario: An expired handle is re-minted once and playback resumes where it was
    Given a native video preview of "holiday.mp4" is paused at 42 seconds
    And its content handle has expired
    When the video reports a media error
    Then the client should mint exactly one new content handle
    And the video should resume at 42 seconds and stay paused

  Scenario: A preview whose new handle also fails shows the error
    Given a native video preview of "holiday.mp4" whose content handle has expired
    And minting a new content handle succeeds but the new handle is refused too
    When the video reports a media error
    Then the client should not mint a third content handle
    And the preview should show the error from the active label table

  # Source
  # - apps/filesquid/ui/resources/srs.md UI-S-014
  # - apps/filesquid/crates/api/resources/srs.md API-S-004, API-S-015, API-S-018
  # - resources/prd.md F-056
