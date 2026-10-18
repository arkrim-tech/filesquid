# F-056: Single FileSquid API
# SRS-F-056: Single FileSquid API (media elements read file bytes through short-lived content handles)
# F-057: Desktop and server hosts
# API-S-004: Every call is authenticated and authorised before files, SQL or keys are touched
# API-S-005: Responses carry opaque handles, never raw filesystem paths or SQL
# API-S-017: Every authorisation refusal and every credential issue is journaled
# API-S-018: Media elements read file bytes through short-lived, read-only content handles
# DE-S-005: Every API request through the scheme carries a per-launch token, except the liveness probe and the content route
# UI-S-014: Content handles are the only credential the UI may put in a URL
#
# Step definitions arrive with Task API-F1.3; until then these scenarios are reported as skipped.
Feature: Content handles for media elements
  As a library owner
  I want my previews and videos to play in native media elements without my token appearing in any URL
  So that a leaked preview address exposes one file for minutes, not my library for the session

  Background:
    Given the owner's library holds a 100 MB video "holiday.mp4" at version v2
    And the owner's client holds a valid credential for the library

  # Happy path
  Scenario: An authorised client mints a content handle for one file version
    When the client mints a content handle for "holiday.mp4" at version v2
    Then the answer should carry a handle, its content URL and an absolute expiry in RFC 3339 UTC
    And the handle should be 43 base64url characters without padding
    And the content URL should contain neither a filesystem path nor the client's token
    And the activity journal should record one "api.content_handle.issued" entry without the handle

  Scenario: A media element reads a byte range through the content handle
    Given the client has minted a content handle for "holiday.mp4" at version v2
    When a media element requests bytes 0 to 1048575 through the content URL without an Authorization header
    Then the answer should be 206 with "Content-Range" "bytes 0-1048575/104857600"
    And the answer should carry "Accept-Ranges" "bytes"
    And the answer should carry "Content-Type" "video/mp4"

  Scenario: A HEAD request through the content handle describes the file without a body
    Given the client has minted a content handle for "holiday.mp4" at version v2
    When a media element sends HEAD through the content URL
    Then the answer should be 200 with an empty body
    And the answer should carry "Accept-Ranges" "bytes"
    And the answer should carry "Content-Type" "video/mp4"

  Scenario: An unsatisfiable range keeps its 416 answer
    Given the client has minted a content handle for "holiday.mp4" at version v2
    When a media element requests bytes 104857600 to 104857700 through the content URL
    Then the answer should be 416 with "Content-Range" "bytes */104857600"

  Scenario Outline: Every content answer carries the privacy headers
    Given the client has minted a content handle for "holiday.mp4" at version v2
    When a media element makes a <kind> request through the content URL
    Then the answer should carry "Referrer-Policy" "no-referrer"
    And the answer should carry "Cache-Control" "private, no-store"
    And the answer should carry "X-Content-Type-Options" "nosniff"
    And the answer should carry "Content-Security-Policy" "default-src 'none'; sandbox"

    Examples:
      | kind           |
      | ranged         |
      | HEAD           |
      | unsatisfiable  |
      | refused        |

  # Edge cases
  Scenario: Playback keeps the handle alive while the idle window is not exceeded
    Given the client has minted a content handle for "holiday.mp4" at version v2
    When a media element requests a range every 240 seconds for 50 minutes
    Then every range request should be answered with 206

  Scenario: A handle expires after 300 idle seconds
    Given the client has minted a content handle for "holiday.mp4" at version v2
    When 301 seconds pass without a request through the content URL
    Then the next request through the content URL should be refused as forbidden with reason "invalid_credentials"

  Scenario: A handle expires 3,600 seconds after minting even while in use
    Given the client has minted a content handle for "holiday.mp4" at version v2
    When a media element requests a range every 60 seconds for 3,601 seconds
    Then the first request after 3,600 seconds should be refused as forbidden with reason "invalid_credentials"

  Scenario: A range read that started before expiry is not cut off
    Given the client has minted a content handle for "holiday.mp4" at version v2
    And a media element has started a 16 MiB range read 1 second before the handle expires
    When the handle expires while the answer is being sent
    Then the media element should receive all 16 MiB of the answer

  Scenario: The 1,025th handle evicts the least recently used one
    Given the client has minted 1,024 content handles
    When the client mints one more content handle
    Then the least recently used handle should be refused as forbidden with reason "invalid_credentials"
    And the other 1,024 handles should still be answered

  # Error cases
  Scenario Outline: Unusable handles get one identical denial that reveals nothing about the file
    When a media element requests a range through <handle>
    Then the answer should be 403 forbidden with reason "invalid_credentials" and not retryable
    And the answer should never be 401 or 404
    And the answer should be byte for byte the same as for an unknown handle
    And no file should have been read
    And the activity journal should record one "api.auth.denied" entry with reason "invalid_credentials"
    And no log line or journal entry should contain the presented handle

    Examples:
      | handle                                            |
      | an unknown handle                                 |
      | a malformed handle                                |
      | a missing handle (an empty path segment)          |
      | an extra path segment after the handle            |
      | an expired handle                                 |
      | a handle for a file its minter may no longer read |
      | a handle whose issuing credential has ended       |

  Scenario: A handle grants nothing beyond its file version
    Given the client has minted a content handle for "holiday.mp4" at version v2
    When a client presents the handle as a Bearer token on another API route
    Then the call should be refused as forbidden with reason "invalid_credentials"
    And the content URL should refuse a write method

  Scenario: Minting is authorised exactly like reading the file
    Given a client whose credential may not read "holiday.mp4"
    When that client tries to mint a content handle for "holiday.mp4" at version v2
    Then the minting should get the same refusal as a ranged read of "holiday.mp4" by that client

  Scenario: Handles end with the desktop launch that minted them
    Given the client has minted a content handle for "holiday.mp4" at version v2 on the desktop
    When the desktop app is relaunched
    Then a request through the old content URL should be refused as forbidden with reason "invalid_credentials"

  Scenario: A HEAD denial matches a GET denial without a body
    When a media element sends HEAD through an unknown handle
    Then the answer should be 403 with the same headers as a GET denial and an empty body

  Scenario Outline: On filesquid-server a handle ends with its credential or permission
    Given a filesquid-server where Ana has minted a content handle for "holiday.mp4" at version v2
    When <event>
    Then Ana's next request through the content URL should be refused as forbidden with reason "invalid_credentials"
    And the activity journal should record one "api.auth.denied" entry with reason "invalid_credentials"

    Examples:
      | event                                                 |
      | Ana logs out                                          |
      | Ana's session ends                                    |
      | the server token that minted the handle is rotated    |
      | the server restarts                                   |
      | Ana's permission to read "holiday.mp4" is withdrawn   |
