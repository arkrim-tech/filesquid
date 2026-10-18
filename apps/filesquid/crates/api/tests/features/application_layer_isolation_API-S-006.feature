# API-S-006: Application meaning is implemented once in filesquid-api, which links no Tauri host APIs, libvips, FFmpeg or SQLite driver and performs no storage I/O itself
Feature: Application-layer isolation of filesquid-api
  As a FileSquid maintainer
  I want the application layer to link only the modules and portable crates
  So that every host, including the future browser host, reuses one API without native or storage code

  # Happy path
  Scenario: The application layer is the filesquid-api crate
    Given the FileSquid Cargo workspace
    When I look up the application-layer crate
    Then it should be the package filesquid-api in crates/api
    And its module SRS should state the API-S-006 boundary

  # Cargo features are not the only boundary: every feature set is checked
  Scenario Outline: The resolved graph holds no forbidden crate beyond the recorded deviations
    Given the resolved runtime graph of filesquid-api with the <features> feature set
    When I check it against the application-layer rules
    Then it should hold no Tauri, libvips, FFmpeg, SQLite driver, storage or network client crate except the deviations pinned by the CL-0.1 Conflict Artifact

    Examples:
      | features            |
      | default             |
      | no-default-features |
      | all-features        |

  # Edge case
  Scenario Outline: The FileSquid modules may be linked
    Given a dependency on <module>
    When I check it against the application-layer rules
    Then the dependency should be allowed

    Examples:
      | module             |
      | filesquid-core     |
      | filesquid-ocfl     |
      | filesquid-metadata |
      | filesquid-c2pa     |

  # Error case
  Scenario Outline: A forbidden dependency fails the guard
    Given a dependency on <crate>
    When I check it against the application-layer rules
    Then the dependency should be rejected citing API-S-006

    Examples:
      | crate          |
      | tauri          |
      | libvips        |
      | ffmpeg-sys     |
      | rusqlite       |
      | libsqlite3-sys |
      | sqlx-sqlite    |
      | fs4            |
      | reqwest        |

  # Happy path
  Scenario: The application layer performs no new direct storage I/O
    Given the source files of filesquid-api
    When I scan them for direct storage I/O
    Then only the files pinned by the CL-0.1 Conflict Artifact should perform it

  # Error case
  Scenario Outline: A direct storage call fails the source guard
    Given a source line "<line>"
    When I scan it for direct storage I/O
    Then the line should be reported as storage I/O

    Examples:
      | line                              |
      | std::fs::write(path, bytes)?;     |
      | use std::{fs, io};                |
      | tokio::fs::read(path).await       |
      | fs4::available_space(dir)         |

  # Source
  # - apps/filesquid/crates/api/resources/srs.md API-S-006
  # - resources/prd.md TC-005
  # - resources/architecture/025-client-side-filesquid-engines.md Decision 6
