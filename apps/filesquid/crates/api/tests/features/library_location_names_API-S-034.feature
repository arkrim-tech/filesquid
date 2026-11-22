# API-S-034: Folder and volume names — on the desktop host, where the caller is the owner on the
# owner's own device, the location-choices answer for a chosen folder also carries folder_name (the
# folder's own name only, never a parent folder or the path) and volume_name, each null when the
# host cannot read it; every other host answers no name. API-S-038: the library facts answer the
# same two names on the desktop. DE-S-017: the desktop host reads them. The names never reach a
# journal entry, a refusal, an event or a log line.
# Owner decision 2026-11-02, verbatim: "RE: 5 - the desktop user can see folder name and volume so
# they can better manage their own private desktop".
# Bound by Task API-F2.31 (support/location_names_steps.rs) over the library-routes rig with a
# scripted host dialog; the desktop host's own reader is covered by the desktop and native tests.
Feature: The owner's library folder and volume names on the desktop
  As the owner of a desktop installation
  I want to see the name of the folder I chose and the volume it is on
  So that I can manage my own libraries on my own computer

  # Happy path
  Scenario: A chosen folder is answered with its name and volume
    Given location-names desktop host whose dialog chooses the folder "Family archive" on the volume "Photos SSD"
    When location-names owner asks for a location for the purpose create
    Then location-names answer should carry a location ID, folder_name "Family archive" and volume_name "Photos SSD"
    And location-names answer should carry no path and no parent folder

  Scenario: The library facts name the library's folder and volume
    Given location-names library opened from the folder "Family archive" on the volume "Photos SSD"
    When location-names owner reads the library facts
    Then location-names facts should carry folder_name "Family archive" and volume_name "Photos SSD"

  # Edge cases
  Scenario: A volume whose name cannot be read is null
    Given location-names desktop host that cannot read the volume name of the chosen folder
    When location-names owner asks for a location for the purpose open
    Then location-names answer should carry folder_name and a null volume_name

  Scenario: A library folder that cannot be reached now has no names
    Given location-names remembered library whose folder cannot be reached
    When location-names owner reads the library facts
    Then location-names facts should carry a null folder_name and a null volume_name

  Scenario: A host that is not the owner's desktop answers no name
    Given location-names host without folder names
    When location-names caller reads the library facts
    Then location-names facts should carry a null folder_name and a null volume_name

  # Privacy
  Scenario: The names never reach the journal, a refusal or an event
    Given location-names desktop host whose dialog chooses the folder "Tax 2026" on the volume "Backup"
    When location-names owner asks for a location and then creates a library there
    Then location-names journal, events and log lines should contain neither "Tax 2026" nor "Backup"
    And location-names journal should name only the opaque location and library IDs
