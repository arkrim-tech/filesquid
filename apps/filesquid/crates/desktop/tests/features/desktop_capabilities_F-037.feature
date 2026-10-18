# F-037: Setup, updates and diagnostics
# SRS-F-037: Setup, updates and diagnostics
# DE-S-001: The desktop serves the FileSquid API in-process through a Tauri custom URI scheme with no TCP listener; any remaining Tauri command MUST be typed and explicitly authorized by a deny-by-default Tauri capability
# DE-S-002: Capabilities are deny-by-default; no fs/shell/sql plugins
# DE-S-006: The webview MUST receive opaque handles and API responses, never raw filesystem paths or SQL
Feature: Desktop capabilities are deny-by-default
  As the owner using the FileSquid desktop app
  I want the app window to hold only the native permissions it needs
  So that a page in the window can never read my files, run programs or query databases behind the API's back

  Background:
    Given the desktop host's shipped capability set

  # Happy path
  Scenario: The shipped capability set passes the deny-by-default policy
    Then the capability set should have no policy violations

  Scenario: The main window may listen for FileSquid events and nothing else
    Then the main window should be granted exactly "core:event:allow-listen" and "core:event:allow-unlisten"
    And no Tauri command of the app should be defined

  # Error cases: every way of widening the window's access is refused
  Scenario Outline: A capability that grants file, SQL, program or dialog access is refused
    When a capability grants "<permission>" to the main window
    Then the policy should refuse it as the forbidden plugin permission "<permission>"

    Examples:
      | permission          |
      | fs:default          |
      | sql:default         |
      | shell:allow-execute |
      | dialog:allow-open   |

  Scenario: A capability that would hand the window filesystem paths is refused
    When a capability grants "core:path:default" to the main window
    Then the policy should refuse it as the ungranted permission "core:path:default"

  # Edge cases
  Scenario: A capability for every window is refused
    When a capability names the window "*"
    Then the policy should refuse it as the unknown window "*"

  Scenario: A capability for remote pages is refused
    When a capability grants event listening to remote pages
    Then the policy should refuse it for its remote context

  Scenario: A Tauri command defined next to the handler in lib.rs is refused
    When lib.rs defines the Tauri command "health_ping"
    Then the policy should refuse the command "health_ping" in lib.rs

  Scenario: A Tauri command that no capability authorizes is refused
    When a submodule defines the Tauri command "read_any_file"
    Then the policy should refuse the unlisted command "read_any_file"
