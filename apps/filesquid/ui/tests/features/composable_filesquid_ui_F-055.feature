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

  # Source
  # - resources/prd.md F-055
  # - resources/srs.md SRS-F-055
  # - resources/architecture/028-filesquid-ui-packages-and-panel-composition.md
