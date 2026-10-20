# SRS-TC-005: Shared Rust graph, hosted mailbox isolation and OCFL writer (DE-S-015; ADR 001, ADR 033 amendment 2026-10-18)
Feature: The native-processing library boundary
  As a developer
  I want native storage and background workers to live in filesquid-native
  So that each host links only what its privacy boundary allows and the Tauri package stays a thin host

  Background:
    Given the workspace's Cargo metadata

  # Happy path
  Scenario: The desktop host links the desktop-only native processing
    Then "filesquid-desktop" should link "filesquid-native" with the feature "desktop"

  # Edge case
  Scenario: The server links filesquid-native with default features only
    Then "filesquid-server" should link "filesquid-native" with no feature

  # Error case
  Scenario: No crate but the two hosts links filesquid-native
    Then no package other than "filesquid-desktop" and "filesquid-server" should link "filesquid-native"

  # Error case
  Scenario: Only filesquid-native links a SQLite driver
    Then no package other than "filesquid-native" should link "rusqlite"
