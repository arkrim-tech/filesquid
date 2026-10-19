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
  Scenario Outline: The resolved graph holds no forbidden crate
    Given the resolved runtime graph of filesquid-api with the <features> feature set
    When I check it against the application-layer rules
    Then it should hold no Tauri, libvips, FFmpeg, SQLite driver, storage, native-processing, network client or alternative async runtime crate

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
      | filesquid-prov     |

  # Error case
  Scenario Outline: A forbidden dependency fails the guard
    Given a dependency on <crate>
    When I check it against the application-layer rules
    Then the dependency should be rejected citing API-S-006

    Examples:
      | crate                 |
      | tauri                 |
      | libvips               |
      | ffmpeg-sys            |
      | rusqlite              |
      | libsqlite3-sys        |
      | sqlx-sqlite           |
      | fs4                   |
      | filesquid-native      |
      | reqwest               |
      | async-std             |
      | smol                  |
      | async-executor        |
      | async-global-executor |
      | async-io              |
      | async-net             |
      | actix-rt              |
      | glommio               |
      | monoio                |
      | compio-runtime        |
      | tokio-uring           |

  # Happy path
  Scenario: The application layer performs no direct storage I/O
    Given the source files of filesquid-api
    When I scan them for direct storage I/O
    Then no source file should perform it

  # Error case
  # In the source-line examples, QUOTE stands for a double quote.
  Scenario Outline: A direct storage call fails the source guard
    Given a source line "<line>"
    When I scan it for direct storage I/O
    Then the line should be reported as storage I/O

    Examples:
      | line                                         |
      | std::fs::write(path, bytes)?;                |
      | use std::{fs, io};                           |
      | tokio::fs::read(path).await                  |
      | fs4::available_space(dir)                    |
      | let u = QUOTEhttps://a.exampleQUOTE; std::fs::remove_file(p)?; |

  # Edge case
  Scenario Outline: Comments and strings that name a storage call pass the source guard
    Given a source line "<line>"
    When I scan it for direct storage I/O
    Then the line should not be reported as storage I/O

    Examples:
      | line                                   |
      | // never call std::fs here             |
      | /* tokio::fs is forbidden */ let x = 1; |
      | let msg = QUOTEstd::fs is forbiddenQUOTE;    |

  # Source
  # - apps/filesquid/crates/api/resources/srs.md API-S-006
  # - resources/prd.md TC-005
  # - resources/architecture/025-client-side-filesquid-engines.md Decision 6
