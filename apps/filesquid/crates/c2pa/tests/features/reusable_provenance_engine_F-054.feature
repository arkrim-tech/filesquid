# F-054: C2PA module
# SRS-F-054: C2PA module
Feature: Reusable filesquid-c2pa engine
  As a developer adding C2PA to my own application
  I want to integrate filesquid-c2pa as one FOSS library
  So that I can read, validate, mint and edit C2PA manifests without installing the FileSquid app

  Scenario: Read a manifest store
    Given a JPEG with an embedded C2PA manifest store
    When I read it with filesquid-c2pa
    Then the active manifest, its claim and its assertions should be returned

  # C2PA-S-001: manifest stores in sidecar files and by remote reference
  Scenario: Read a sidecar manifest store
    Given a sidecar manifest store with an ingredient manifest and an active manifest
    When I read the sidecar with filesquid-c2pa
    Then the active manifest should be the last manifest in the store
    And its v2 claim and its CBOR, JSON and embedded-file assertions should be returned

  Scenario: Read a legacy v1 claim
    Given a sidecar manifest store whose claim uses the legacy c2pa.claim label
    When I read the sidecar with filesquid-c2pa
    Then the claim should be reported as version 1 with its assertions

  Scenario: Read a remote manifest store through the caller fetcher
    Given a caller fetcher that serves a manifest store
    When I read the remote manifest store "https://example.org/asset.c2pa"
    Then the active manifest should be the last manifest in the store
    And the fetcher should have been asked for a remote manifest

  Scenario: A remote manifest store without a caller fetcher is inaccessible
    Given no network fetcher supplied by the caller
    When I read the remote manifest store "https://example.org/asset.c2pa"
    Then the read should fail with status code "manifest.inaccessible"

  Scenario: A truncated manifest store is rejected
    Given a sidecar manifest store cut short by one byte
    When I read the sidecar with filesquid-c2pa
    Then the read should fail with a truncated JUMBF box error

  Scenario: A manifest store over the size limit is rejected before it is read
    Given a sidecar manifest store one byte larger than the store size limit
    When I read the sidecar with filesquid-c2pa
    Then the read should fail as too large without reading the source

  Scenario: Nesting deeper than the limit is rejected
    Given a sidecar manifest store nested one level deeper than the box depth limit
    When I read the sidecar with filesquid-c2pa
    Then the read should fail with a box depth limit error

  Scenario: A manifest with a broken claim is reported with its status code
    Given a sidecar manifest store whose ingredient claim is not valid CBOR
    When I read the sidecar with filesquid-c2pa
    Then the ingredient manifest should report "claim.cbor.invalid"
    And the active manifest should still be readable

  Scenario: Unknown boxes are reported, not silently dropped
    Given a sidecar manifest store with a vendor box of an unknown type
    When I read the sidecar with filesquid-c2pa
    Then the vendor box should be listed as ignored with an unknown type

  Scenario: Validate against a caller-supplied trust list
    Given a signed asset and a trust list supplied by the caller
    When I validate it with filesquid-c2pa
    Then the result should use C2PA validation status codes
    And an untrusted signing certificate should be reported, not ignored

  Scenario: Mint and sign through a caller-supplied signer
    Given an unsigned PNG and a test signer
    When I mint a manifest with a created action and sign it
    Then the embedded manifest should validate in filesquid-c2pa
    And the official c2pa oracle should read the same manifest

  Scenario: Edits never change a signed manifest in place
    Given a signed asset
    When I record an edit
    Then a new manifest should reference the prior asset as a parent ingredient

  Scenario: Unsupported formats are reported
    Given an asset in a format without a handler
    When I read it with filesquid-c2pa
    Then the result should say the format is unsupported

  Scenario: Stand-alone dependencies
    When I inspect filesquid-c2pa library dependencies
    Then filesquid-ocfl, axum, tauri, sqlite, libvips, ffmpeg, and filesquid-core should be absent from default dependencies
    And no runtime dependency on the official c2pa crate should exist

  @wasm32
  Scenario: Engine builds for the browser
    When I build filesquid-c2pa for wasm32-unknown-unknown with default features off
    Then the build should succeed

  # C2PA-S-009
  @wasm32
  Scenario: Engine builds for the browser with the default crypto backend
    When I build filesquid-c2pa for wasm32-unknown-unknown with default features on
    Then the build should succeed

  # C2PA-S-006: cryptography behind the crypto-backend trait
  Scenario: Cryptography stays behind the crypto backend
    When I inspect the filesquid-c2pa sources
    Then only the crypto backend should name a cryptography crate

  Scenario: The default crypto backend accepts a valid Ed25519 signature
    Given the RFC 8032 Ed25519 test key and signature
    When I verify the signature through the default crypto backend
    Then the signature should be accepted

  Scenario: The default crypto backend rejects a tampered message
    Given the RFC 8032 Ed25519 test key and signature
    When I verify the signature over a different message through the default crypto backend
    Then verification should fail

  # C2PA-S-008: bytes-only core
  Scenario: The core performs no direct file or network access
    When I inspect the filesquid-c2pa sources
    Then no direct filesystem, network or process call should be present

  Scenario: The engine reads through a caller-supplied byte source
    Given a caller-supplied byte source holding "c2pa manifest bytes"
    When the engine reads 8 bytes at offset 5
    Then it should receive "manifest"

  Scenario: Reading past the end of a byte source is reported
    Given a caller-supplied byte source holding "short"
    When the engine reads 8 bytes at offset 2
    Then an unexpected end of data should be reported

  Scenario: Remote fetches without a caller fetcher are refused
    Given no network fetcher supplied by the caller
    When the engine requests a remote manifest of at most 16 bytes
    Then the request should be refused as offline

  Scenario: Oversized remote responses are rejected
    Given a caller fetcher that returns 32 bytes
    When the engine requests a remote manifest of at most 16 bytes
    Then the response should be rejected as too large

  # C2PA-S-007: official tools are interoperability oracles only
  Scenario: The official c2pa tools are oracles, not dependencies
    When I inspect filesquid-c2pa library dependencies
    Then no runtime dependency on the official c2pa crate should exist
    And the c2patool oracle harness should report verdicts or a skip reason

  # SRS-F-054: the module pins a C2PA specification version
  Scenario: The C2PA specification version is pinned
    When I inspect the filesquid-c2pa specification pin
    Then the library and the README should both name C2PA 2.4

  Scenario: Content Credentials panel plugs into a file browser sidebar
    Given a selected signed file in a host file browser
    When the host registers the filesquid-c2pa Content Credentials panel
    Then the panel should show the signer, assertions, and validation status

  Scenario Outline: Content Credentials panel shows each credential state distinctly
    Given a selected <state> file in a host file browser
    When the host registers the filesquid-c2pa Content Credentials panel
    Then the panel should show the <state> credential state
    And the panel should not show any other credential state

    Examples:
      | state       |
      | unsigned    |
      | signed      |
      | invalid     |
      | unavailable |

  # Source
  # - resources/prd.md F-054
  # - resources/srs.md SRS-F-054
  # - apps/filesquid/crates/c2pa/resources/srs.md C2PA-S-001, C2PA-S-006, C2PA-S-007, C2PA-S-008, C2PA-S-009
  # - resources/architecture/026-filesquid-provenance-engine.md
  # - resources/architecture/028-filesquid-ui-packages-and-panel-composition.md
