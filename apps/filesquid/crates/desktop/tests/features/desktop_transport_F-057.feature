# F-057: Desktop and server hosts
# F-056: Single FileSquid API
# SRS-F-057: Desktop and server hosts
# DE-S-005: Every API request through the scheme MUST carry a per-launch token, and the event channel MUST sit behind the same client interface as the server's
# DE-S-001: The desktop serves the FileSquid API in-process through a Tauri custom URI scheme with no TCP listener
Feature: Desktop transport: per-launch token, event channel and ranged reads
  As the owner using the FileSquid desktop app
  I want previews read in byte ranges and live updates pushed to my window, all without a network port
  So that large files open quickly and nothing outside the app can reach my library

  Background:
    Given the desktop host has started with a new per-launch token

  # Happy path
  Scenario: An app page reads the end of a 100 MB file as partial content
    When an app page reads the last 100 bytes of the 100 MB test file with the token
    Then the read should succeed as partial content
    And the answer should name the range "bytes 104857500-104857599/104857600"
    And the answer should carry exactly the requested bytes

  Scenario: An app window receives a published event
    Given an app window subscribed to the event channel
    When a process update is published
    Then the window should receive a "process.update" event

  # Edge cases
  Scenario: An open-ended range is capped so memory stays bounded
    When an app page reads the 100 MB test file from its first byte with the token
    Then the read should succeed as partial content
    And the answer should name the range "bytes 0-8388607/104857600"

  Scenario: The whole 100 MB file can be read in capped ranges
    When an app page reads the whole 100 MB test file range by range with the token
    Then it should have read 104857600 bytes in 13 reads

  Scenario: A window that fell behind is told to reread
    Given an app window subscribed to the event channel that is not reading
    When one more event than the buffer holds is published
    Then the window should first receive an "events.resync" hint for 1 missed event

  Scenario: A response larger than the scheme buffer is refused, not buffered
    When the host answers with a body one byte over its buffer cap
    Then the answer should be the typed error "internal"

  # Error cases
  Scenario: A read without the per-launch token is refused
    When a page reads the last 100 bytes of the 100 MB test file without the token
    Then the answer should be the typed error "unauthorized"

  Scenario: A range past the end of the file is refused
    When an app page reads past the end of the 100 MB test file with the token
    Then the answer should be "range not satisfiable" naming the length 104857600

  Scenario: An app window cannot be navigated to a foreign site
    When an app window is asked to navigate to "https://example.com/"
    Then the navigation should be refused
    And the activity journal should record "desktop.navigation.refused" without the address

  # Source
  # - resources/prd.md F-056, F-057
  # - resources/srs.md SRS-F-057: Desktop and server hosts
  # - apps/filesquid/crates/desktop/resources/srs.md DE-S-001, DE-S-005
  # - resources/architecture/030-filesquid-api-and-transports.md Decision 4 (event channel), Decision 6 (errors), S1 results (buffered bodies, item (a))
