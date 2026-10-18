# OCFL-S-020: Opaque bytes; the dual profile is a caller. The engine MUST store the content bytes its caller supplies without reading them as anything but bytes, and logical paths as given within the OCFL 1.1 path rules; owner-readable local media and ciphertext hosted packages are caller transforms, not engine object types, and over ciphertext the inventory MUST stay structurally readable OCFL 1.1.
# F-042: OCFL preservation and portable recovery
# TC-002: Endpoint processing and hosted encryption
Feature: The engine stores opaque bytes and the dual profile is a caller
  As a FileSquid client that seals hosted packages with its own key
  I want the OCFL engine to store whatever bytes and names I give it
  So that owner-readable and ciphertext objects share one engine and both stay valid OCFL

  # Happy path
  Scenario: A sealed object stays structurally readable OCFL
    When a caller writes the content fixture "spec-ex-full" sealed with its own key
    Then the written object should validate with zero errors in filesquid-ocfl
    And its inventory should parse with sha256 digests of the stored bytes
    And the caller should recover the plaintext of every version

  Scenario: The owner-readable profile uses the same engine calls
    When a caller writes the content fixture "spec-ex-full" as plaintext
    Then the written object should validate with zero errors in filesquid-ocfl
    And its inventory should parse with sha256 digests of the stored bytes
    And every version should read back as its plaintext source

  # Edge case
  Scenario: Bytes that look like OCFL files or are not text are stored as content
    When a caller writes files holding every byte value, no bytes, bytes that are not UTF-8 and an OCFL inventory
    Then the written object should validate with zero errors in filesquid-ocfl
    And every version should read back as its plaintext source

  Scenario: A sealed object shows no plaintext
    When a caller writes the content fixture "spec-ex-full" sealed with its own key
    Then no stored key, inventory text or content file should show the plaintext

  # Error cases
  Scenario: A changed sealed file is refused without a key
    Given a caller wrote the content fixture "spec-ex-minimal" sealed with its own key
    When one byte of its sealed content file changes
    Then reading that file should be refused with OCFL code "E092"

  Scenario: The engine has no profile or encryption types
    When I inspect filesquid-ocfl library dependencies
    Then no profile or encryption type or encryption crate should be part of the engine

  # Source
  # - resources/prd.md F-042, TC-002
  # - apps/filesquid/crates/ocfl/resources/srs.md OCFL-S-020
  # - resources/architecture/010-ocfl-1-1-dual-profile.md Decision 2 "inventories and digest sidecars remain structurally readable over ciphertext"
  # - resources/architecture/023-filesquid-ocfl-engine-and-parquet.md "the engine stores opaque bytes the caller supplies"
