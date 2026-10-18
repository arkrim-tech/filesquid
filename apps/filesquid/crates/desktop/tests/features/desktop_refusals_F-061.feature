# F-061: Activity journal and live health monitoring
# SRS-F-061: Activity journal and live health monitoring
# F-037: Setup, updates and diagnostics
# DE-S-002: Capabilities are deny-by-default; the host owns Tauri's IPC scheme and journals every IPC call it or Tauri refuses
# DE-S-003: Logs are stdout JSON; every host-level refusal, warning and error is journaled with redacted fields
Feature: Every refusal in the desktop shell is journaled
  As the owner using the FileSquid desktop app
  I want every call, window, permission or download the app refuses to be recorded in the activity journal
  So that I can see in the Monitor what a page in the window tried to do, without the journal ever holding my files' contents, paths or keys

  # Happy path
  Scenario: An app page may listen for FileSquid events through the host's IPC gate
    When an app page asks to listen for FileSquid events with the invoke key
    Then the host should hand the call to Tauri

  # Error cases
  Scenario Outline: A command outside the policy is refused and journaled without its payload
    When an app page invokes "<command>" with a private path in its payload
    Then the page should be told the command is not allowed
    And the activity journal should record the IPC refusal "command_not_allowed" for "<journal id>"
    And no journal entry should contain the payload or the invoke key

    Examples:
      | command                 | journal id               |
      | plugin:fs\|read_text_file | plugin:fs.read_text_file |
      | plugin:sql\|load          | plugin:sql.load          |
      | plugin:event\|emit        | plugin:event.emit        |
      | health_ping             | health_ping              |

  Scenario: A call with a forged invoke key is refused and journaled without either key
    When a page invokes "plugin:event|listen" with a forged invoke key
    Then the page should be told the command is not allowed
    And the activity journal should record the IPC refusal "invalid_invoke_key" for "plugin:event.listen"
    And no journal entry should contain the payload or the invoke key

  Scenario: Tauri's own access-control refusal of a forwarded call is journaled
    Given a window whose capabilities grant nothing
    When an app page asks to listen for FileSquid events with the invoke key
    Then the page should be told the command is not allowed
    And the activity journal should record the IPC refusal "acl_denied" for "plugin:event.listen"

  # Edge cases
  Scenario: A page that opens a new window is refused and journaled
    When a page asks to open a new window
    Then the activity journal should record "desktop.window.refused" with the code "new_window_denied"

  Scenario: A page that asks for the camera is refused and journaled by kind only
    When a page asks for the camera
    Then the activity journal should record "desktop.permission.refused" with the code "permission_denied"

  Scenario: A download from outside the app is refused and journaled without its address
    When a page starts a download from "https://example.com/holiday-photos.zip"
    Then the download should be refused
    And the activity journal should record "desktop.download.refused" with the code "download_denied"
    And no journal entry should contain "holiday-photos"
