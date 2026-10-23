# API-S-038: A library MUST have a name, set when it is created and renamable by the owner,
# journaled, under the name rules of API-S-019 (1 to 1,024 bytes, not blank; API-S-005 safe).
# GET /api/v1/libraries/{library}/facts (library.read) MUST answer the library's ID and name and
# the free bytes of the volume that holds its storage root, readable by anyone who may read the
# library; free_bytes is null with free_bytes_reason volume_unreadable or not_local when the
# volume cannot be read; an unknown library, or one the caller may not read, is 404 not_found.
# GET /api/v1/libraries/recent (library.read) MUST first keep only the libraries this host opened
# that the caller may read, then list at most 20 of them, newest first, with ID, name, last opened
# time, whether open now and free bytes, never a path, storage key or location ID.
# Hosts read free bytes on Linux and macOS; every other build answers null with
# volume_unreadable until DE-F3.3 (owner decision 5 of 2026-10-23).
# Task API-F2.18 (contract 0.27.0).
Feature: Library facts: name, free space and recent libraries
  As the owner of several libraries
  I want each library to have a name, its free space and a list of the ones I opened
  So that I can tell my libraries apart and reopen one without a folder path

  # Happy path
  Scenario: A library created with a name answers its name and free space
    Given the owner has created the library "Family archive"
    When the owner reads the library's facts
    Then the facts should name it "Family archive" with 1000000 free bytes

  Scenario: A rename is journaled with the library only
    Given the owner has created the library "Family archive"
    When the owner renames the library to "Family archive 1990s"
    Then the facts should name it "Family archive 1990s" with 1000000 free bytes
    And the journal should record one library rename naming the library and not the name

  # Edge cases
  Scenario: A volume that cannot be read answers null free bytes with the reason
    Given the owner has created the library "Remote"
    And the library's volume is not local
    When the owner reads the library's facts
    Then the free bytes should be null because "not_local"

  Scenario: The recent list keeps readable libraries first, then the 20 newest
    Given the owner has opened 22 libraries one after another
    And the caller may not read the newest one
    When the caller lists the recent libraries
    Then 20 libraries should be listed, newest readable first, with no path or location ID

  # Error cases
  Scenario: A name that breaks the rules creates nothing
    When the owner creates a library named with 1025 bytes
    Then the creation should be refused with "invalid_name" and nothing written

  Scenario: An unreadable library is not found
    Given the owner has created the library "Hidden"
    And the caller may not read that library
    When the owner reads the library's facts
    Then the facts should be refused as not found
