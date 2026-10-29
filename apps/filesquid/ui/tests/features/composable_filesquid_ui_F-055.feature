# F-055: Composable filesquid user interface
# SRS-F-055: Composable filesquid user interface
Feature: Composable filesquid user interface
  As an ImageSquid owner and as a developer adopting the filesquid engines
  I want one set of file, metadata, and credential panels
  So that every file fact has one owner and products compose the same trusted UI

  Scenario: Every file fact has one owner
    Given a photo stored through filesquid-ocfl
    When I view it in a file browser with all three panels
    Then the object identifier, logical path and implied folders, versions, version created time, version user and message, digests and fixity status, size, and content location should come from filesquid-ocfl
    And the extension-based file-type label should be a display hint only
    And embedded EXIF, ID3, IPTC, and XMP, format identification, original creation or capture date, Dublin Core and Schema.org description, title, captions, keywords, and rights should come from filesquid-metadata
    And Content Credentials should come from filesquid-c2pa
    And albums, collections, people, favorites, search indexes, and recipes should come from the composite product

  Scenario: UI packages are independent
    When I inspect the ocfl-ui, metadata-ui, and provenance-ui package dependencies
    Then none should depend on another
    And each should build as Svelte components and as Web Components

  Scenario: ImageSquid composes panels instead of reimplementing them
    Given the ImageSquid inspector
    When I open a photo
    Then the Metadata, Content Credentials, Details, and Versions panels should render inside the inspector
    And the Source tab should remain an ImageSquid component

  Scenario: ImageSquid keeps engines off the server
    When I inspect the ImageSquid panel transports
    Then they should use Tauri IPC on the desktop and a WebAssembly worker in the browser
    And none should call the engines over HTTP through Axum

  # Task UI-F2.2: the browser's named slots (ADR 028 Decision 6, UI-S-002)
  Scenario: A panel mounts in a named sidebar slot and receives only the opened file
    Given the file browser with two registered panels "details" and "versions"
    When I open a file
    Then both panels should be shown in their slots in registration order
    And each panel should receive the file reference with library, object, version, path and display name
    And neither panel should hold a reference to the other
    When I close the file
    Then both panels should receive no file and the sidebar should be hidden

  Scenario: A registered preview renderer is tried before the built-in ones
    Given the file browser with a registered renderer for text files
    When I open a text file
    Then the registered renderer should show the file instead of the built-in text preview

  Scenario: The file browser mounts alone
    When I mount the file browser with no panel, no renderer and no distribution
    Then it should list, open and change files through the API only

  # Task UI-F2.3: the file panel, its File info tab (the Details panel) and the Versions view
  # UI-S-003: panels, including the file panel's tabs, plug into named slots and do not depend
  # on each other; UI-S-031: file panel composition (mockups 35 and 25)
  Scenario: Opening a file shows the file panel beside the list under the header
    Given a library file "facade-study.jpg" with four OCFL versions
    When I open the file
    Then the file panel should be shown in the 320 px region at the right, under the full-width header
    And its header should show the "JPG" badge, the display name, the history button and close
    And the File info tab should be selected with a 2 px primary underline

  Scenario: The File info tab shows storage facts only, each owned by filesquid-ocfl
    Given a library file "facade-study.jpg" with four OCFL versions
    When I open the file
    Then File info should show the original file name, extension, MIME type, size in bytes, the SHA-256 and SHA-512 digests, the OCFL object and "v4 of 4" with its UTC time
    And each digest and identifier should be shortened and set in the mono face, with a copy of its whole value
    And the count eyebrow should read "8 properties · read only"
    And the stored name, the logical path and any content location should never appear

  Scenario: A tab mounts in its named slot and only the panel shown reads the file
    Given a file panel with the tabs "details" and "other" and a Versions view
    When I open a file
    Then only the "details" panel should be created and receive the file reference
    When I select the "other" tab
    Then the "other" panel should receive the file reference
    And no panel should hold a reference to another

  Scenario: Filtering File info that matches nothing offers Clear filter
    Given File info is shown for a file
    When I filter it by "copyright"
    Then I should see "No file info matches “copyright”" with "Try another word, or clear the filter."
    When I choose Clear filter
    Then every property should be listed again and the filter should have focus

  Scenario: The panel states keep the header, tabs and filter in place
    Given the storage facts of a file are still loading
    When I open the file
    Then File info should show "Loading file info…" over placeholder rows and a disabled filter
    When the API answers 503 unavailable
    Then File info should show "Could not load the file info", the reason, "Nothing was changed.", the code "unavailable" and Try again

  Scenario: Closing the file panel keeps the file open
    Given the file panel shows a file
    When I close the file panel
    Then the panel region should be hidden
    And the file should stay open in the viewer

  # Task UI-F2.4: the Description tab (the Metadata panel) in its named slot
  # UI-S-003: each file fact is shown by the panel of its owning module; UI-S-022: the tabs
  # share one property table; UI-S-031: Description composition (mockups 28 and 29)
  Scenario: The Description tab shows the facts the metadata module owns, in its own slot
    Given a file panel with the tabs "details" and "description"
    When I select the Description tab
    Then the metadata panel should be created in its Description slot and receive the file reference
    And it should show the file's dcterms: and schema: statements and nothing the OCFL module owns
    And it should share the file info tab's property table, filter and states
    And neither panel should hold a reference to the other

  # Source
  # - resources/prd.md F-055
  # - resources/srs.md SRS-F-055
  # - resources/architecture/028-filesquid-ui-packages-and-panel-composition.md
