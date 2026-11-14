# F-058: White-label distributions
# SRS-F-058: White-label distributions
# DE-S-007: One desktop binary MUST serve every distribution through a Tauri config overlay (product name, identifier, icons, UI directory)
# DE-S-001: The desktop serves the FileSquid API in-process through a Tauri custom URI scheme with no TCP listener; any remaining Tauri command MUST be typed and explicitly authorized by a deny-by-default Tauri capability
# DE-S-002: Capabilities are deny-by-default; no fs/shell/sql plugins
Feature: One desktop binary is branded and packaged through configuration overlays
  As the owner installing FileSquid on a Mac
  I want the app I install to carry the FileSquid name, identifier and icon
  So that I can tell it apart from any other distribution built from the same program

  # Happy path
  Scenario: The FileSquid overlay brands the desktop build
    Given the desktop build configuration with the "filesquid" overlay
    Then the product name should be "FileSquid"
    And the application identifier should be "tech.filesquid.desktop"
    And the main window title should be "FileSquid"
    And the UI directory should be "../../ui/dist"
    And every listed icon should exist, with a macOS icon among them

  Scenario: The macOS installer overlay asks for an early-access disk image
    Given the desktop build configuration with the "filesquid" and "macos-dmg" overlays
    Then the build should produce installers
    And the only installer should be "dmg"
    And the app should be signed ad hoc, without a Developer ID
    And no updater should be configured

  # Edge cases
  Scenario: Without the installer overlay no installer is built
    Given the desktop build configuration with the "filesquid" overlay
    Then the build should produce no installers

  Scenario: Another distribution reuses the installer overlay unchanged
    Given the desktop build configuration with another distribution's overlay and the "macos-dmg" overlay
    Then the product name should be "ImageSquid"
    And the only installer should be "dmg"
    And the app should be signed ad hoc, without a Developer ID

  Scenario Outline: The window's permissions stay the same under the overlays on <platform>
    Given the desktop build configuration with the "filesquid" and "macos-dmg" overlays
    When the capability policy checks that configuration for "<platform>"
    Then the configuration should pass the capability policy

    Examples:
      | platform |
      | macos    |
      | linux    |
      | windows  |

  # Error case
  Scenario: An overlay that gives the window more permissions is refused
    Given the desktop build configuration with the "filesquid" overlay and one that enables the capability "wide"
    When the capability policy checks that configuration for "macos"
    Then the policy should refuse the enabled capability list "default, wide"
