# Project Requirements Document: ImageSquid

<img src="./assets/ImageSquid-logo.svg" alt="ImageSquid logo" width="96" height="96">

Status: Draft. This PRD holds ImageSquid's photo-specific requirements under the same F- IDs as the FileSquid PRD; platform requirements stay in the FileSquid PRD. Requirements use RFC 2119.

# Executive Summary

The ImageSquid distribution MUST provide a highly performant, local-first photo and video library over FileSquid. Mature ImageSquid scope covers every feature family that mainstream photo managers offer, with safe migration out of Apple Photos, Google Photos and Microsoft-platform collections. Proprietary rendering, unavailable source metadata and unsupported platform APIs require explicit limits and equivalent outcomes; they cannot be silently called full compatibility. Most users are expected to use ImageSquid as an installed PWA on an iPhone or Android phone, so phone photo sync and in-app camera capture ([F-036](#f-036-browser-and-mobile-web-experience)) are first-class requirements. Every backend capability ImageSquid needs (derivatives, editing recipes, people and places, migration routes, AI connections, publication) is a FileSquid module behind the API (FileSquid U-25).

ImageSquid is a distribution of FileSquid: a UI layer of views, design tokens (light and dark), a label table, brand assets and a distribution manifest. It contains no Rust, no server code and no API of its own. It calls only the FileSquid API through `FileSquidClient` and composes `@filesquid/ui` custom elements ([IS-S-001](./srs.md)).

## How ImageSquid requirements relate to FileSquid

- Each requirement below keeps the ID of the FileSquid requirement it presents (for example F-003). FileSquid implements the capability behind its API and owns the backend acceptance criteria in the FileSquid PRD. This PRD states what ImageSquid's views MUST do.
- For requirements whose user-facing criteria are photo-specific (F-003, F-004, F-007, F-011–F-023, F-028, and the phone photo sync and camera criteria of F-036), the view criteria live here and ImageSquid owns them. FileSquid's PRD keeps only the platform capability and FileSquid's own default UI for the same ID (trimmed 2026-10-16). For every other requirement, ImageSquid MUST meet FileSquid's criteria for the views it shows.
- A capability ImageSquid needs that FileSquid lacks MUST be added to FileSquid as a module behind the API, never to ImageSquid.
- FileSquid documents and IDs are named in plain text, not linked.
- ImageSquid's UI surfaces (SRS-UI-001–006, SRS-UI-008), screenshot decisions and distribution rules are in the [ImageSquid SRS](./srs.md). Execution order is in the [ImageSquid plan](./plan.md).

## Release order and dependency gates

ImageSquid is the second consumer release. It follows the FileSquid release (FileSquid's default UI with the `ocfl` and `metadata` modules) and FileSquid's encrypted backup and sync; FileSquid's `c2pa` module ships with the ImageSquid release. After the ImageSquid release, the owner chooses the order of the later milestones (Arkrim, IIIF/GLAM, ProofMode). Copied from the FileSquid PRD release table (2026-10-16; updated 2026-10-18 for the C2PA move, condensed here):

| Release or gate | Prerequisites | Scope and objective | Exit gate |
|---|---|---|---|
| ImageSquid release (second consumer release) | FileSquid release and encrypted backup and sync | The ImageSquid distribution ([F-058](#f-058-white-label-distributions)) with photo views and the FileSquid media, editing, people, migration, AI and publication modules it enables, including FileSquid's Content Credentials (C2PA) module, which ships with this release. Its internal gates follow [C-040](./product-vision.md#parity-preservation-and-sequence) in this order: **(a) ownership and dependable organization:** F-003 progressive grid/viewer/basic-video/selection, F-004 albums and galleries, F-005, F-006 smart gallery predicates (local metadata, `dcterms:` / `schema:`, camera, album, status), F-007 non-semantic media/activity views, F-016 burst review, migration package M-A; **(b) editing:** F-011–013 everyday photo editing/RAW and corresponding mobile basic edits; **(c) discovery:** F-019–021, F-045 compatible recognition/discovery task routes and intelligent F-006/007/039, including People/Pets/Trips/Months/Years and semantic collections; **(d) connectivity:** shared photo surfaces of F-033/034/036/039; **(e) storytelling:** F-022/023 storytelling, prepared slideshows, print/PDF and authorized project adapters; **(f) advanced media:** F-014–018 and remaining advanced F-036; **(g) parity audit:** complete F-024/028/037/040 integration, F-002 native camera/phone adapters, migration refinements, mature macOS/Windows/Linux/browser support, locale/accessibility and performance audit. Recognition-dependent predicates/curation and synchronized definitions wait for gates (c) and (d) | (a) large-library concurrent-work benchmark, RC-009 scan and RC-010/013 metadata restoration/privacy; (b) render/revert/golden-image and original-hash checks; (c) labeled search/recognition corpus with approved relevance/error thresholds, correction and permission/private-content exclusion tests; (d) RC-006 for shared photo surfaces; (e) reopen/edit/export presentations, correct prepared-slide/timing and print-layout verification, music/provider rights; (f) per-format preservation/edit/export contracts with capability-dependent gaps registered; (g) RC-001–013 and full item-level photo-manager parity/adopted-scope review; unresolved proprietary gaps remain disclosed |

# Functional Requirements

## F-003: Gallery, selection and viewing

**FileSquid requirement:** FileSquid PRD F-003, same ID. FileSquid implements the capability behind the FileSquid API; ImageSquid presents it through its own views.

**Priority:** Mandatory. **User Story:** As a viewer, I want responsive browsing and inspection of the collection.

**Delivered by:** ImageSquid distribution; capabilities as FileSquid modules (FileSquid U-25).

**ImageSquid surfaces:** [SRS-UI-001](./srs.md#srs-ui-001-library-chrome).

**Tests:** [`gallery_selection_and_viewing_F-003.feature`](../tests/features/gallery_selection_and_viewing_F-003.feature).

### Acceptance Criteria

ImageSquid's own view criteria (moved from FileSquid F-003 on 2026-10-16). FileSquid F-003 now states only the platform capability these views call and FileSquid's default UI.

- The UI MUST provide All Photos, date/day navigation, Collections, adjustable thumbnail size, square/original proportions, adjacent-item navigation, full screen, zoom and pan.
- Video MUST provide play/pause/scrub; supported motion media MUST include playback/sound, with user-controlled automatic playback. HDR MUST be capability-detected with an identified SDR fallback.
- Selection MUST support individual, range, non-contiguous and all-current-result items, including keyboard alternatives; batch operations MUST identify the effective selection scope.
- Overlays MUST optionally show titles, technical indicators and face names. Display exclusions for screenshots and received/shared items MUST not delete or alter underlying membership.
- Virtualization MUST retain focus, selection and accessible item semantics as the visible window changes.
- Gallery queries and rendered items MUST remain bounded. The viewer MUST paint suitable available thumbnails/display derivatives promptly and refine to the requested quality, without requiring complete-library preprocessing or repeatedly decoding full originals for ordinary navigation. Orientation, color interpretation and edit/resource version MUST be correct at every displayed quality.
- The application MUST prefetch media immediately before and after the visible/current view within declared finite item, byte and time limits. It MUST adapt to direction, jumps, bandwidth and memory pressure, deduplicate work, cancel obsolete speculative jobs and reject stale results without losing ordering, focus or selection.
- Thumbnail, display/decoded-media and suitable view caches MUST use versioned content/view/permission keys, explicit invalidation and finite memory/disk budgets under NFR-003/004. Missing, evicted or failed media MUST show an honest state and recovery path; a spinner or placeholder alone MUST NOT satisfy NFR-002 useful-image timing.

## F-004: Albums, folders and navigation

**FileSquid requirement:** FileSquid PRD F-004, same ID. FileSquid implements the capability behind the FileSquid API; ImageSquid presents it through its own views.

**Priority:** Mandatory. **User Story:** As a curator, I want flexible albums without duplicating originals.

**Delivered by:** ImageSquid distribution; capabilities as FileSquid modules (FileSquid U-25).

**ImageSquid surfaces:** [SRS-UI-001](./srs.md#srs-ui-001-library-chrome).

### Acceptance Criteria

ImageSquid's own view criteria (moved from FileSquid F-004 on 2026-10-16). FileSquid F-004 now states only the platform capability these views call and FileSquid's default UI.

- Users MUST create empty/from-selection collections (UI: albums), rename, populate by command/drag, duplicate, merge and delete them. A collection MUST belong to exactly one library. A file's bytes MUST live in exactly one collection. Multiple album membership without duplicating originals MUST be a virtual collection (UI: gallery) or other non-storage membership, not a second collection store (FileSquid U-05).
- Collection removal/deletion MUST retain files. Library deletion of a file MUST be a distinct labeled action.
- Collections MUST support manual order, title/date ascending/descending sort, cover selection, duplication/merge with deterministic membership handling, and manual/sorted collection ordering.
- Nested folders, pinned destinations, reorderable collections/sidebar sections, collapse/expand, collection thumbnail-size controls and compact/thumbnail presentation MUST persist across restarts. Keyboard equivalents MUST exist for reordering.
- A gallery MUST be presented as either a smart gallery (saved query) or a hand-picked gallery (explicit list). **Add To** MUST offer the file's album and every hand-picked gallery; adding a file that already has a home album to a second album MUST create or extend a hand-picked gallery, and the UI MUST say so the first time in a session rather than silently copying. Hand-picked galleries MUST offer the same manual order, sort, cover, duplicate, merge and delete controls as albums (FileSquid F-004).
- **Undo** and **Redo** MUST be available from the Edit menu and the keyboard for add to or remove from an album or gallery, reorder, rename, album or gallery deletion, favorite and keyword changes, with the action named (for example "Undo Remove 3 Photos from Album"). Undo MUST restore through the FileSquid compensating command, never by editing history (FileSquid F-004, NFR-006).

## F-005: Favorites, metadata and keywords

**FileSquid requirement:** FileSquid PRD F-005, same ID. FileSquid implements the capability behind the FileSquid API; ImageSquid presents it through its own views.

**Priority:** Mandatory. **User Story:** As an owner, I want searchable descriptions and consistent batch corrections.

**Delivered by:** FileSquid (`metadata` module, favorites and keywords through the API). The ImageSquid distribution presents the photo information panel and Keyword Manager (FileSquid U-25).

**ImageSquid surfaces:** [SRS-UI-001](./srs.md#srs-ui-001-library-chrome), [SRS-UI-002](./srs.md#srs-ui-002-media-inspector) (favorites in the sidebar; metadata and keywords in the inspector Metadata tab).

**Acceptance:** ImageSquid MUST present this capability only through the FileSquid API, MUST NOT reimplement it, and MUST meet the acceptance criteria of FileSquid F-005 for every view it shows.

## F-006: Smart galleries and metadata search

**FileSquid requirement:** FileSquid PRD F-006, same ID. FileSquid implements the capability behind the FileSquid API; ImageSquid presents it through its own views.

**Priority:** Mandatory. **User Story:** As a curator, I want collections and searches that update as my library changes.

**Delivered by:** FileSquid (virtual collections and search through the API). The ImageSquid distribution presents rule-based virtual collections as smart galleries (saved queries) (FileSquid U-25).

**ImageSquid surfaces:** [SRS-UI-001](./srs.md#srs-ui-001-library-chrome) (Smart Queries sidebar group and search).

**Acceptance:** ImageSquid MUST present this capability only through the FileSquid API, MUST NOT reimplement it, and MUST meet the acceptance criteria of FileSquid F-006 for every view it shows.

## F-007: Automatic and activity collections

**FileSquid requirement:** FileSquid PRD F-007, same ID. FileSquid implements the capability behind the FileSquid API; ImageSquid presents it through its own views.

**Priority:** Mandatory. **User Story:** As a viewer, I want useful ways back into my collection.

**Delivered by:** ImageSquid distribution; capabilities as FileSquid modules (FileSquid U-25).

**ImageSquid surfaces:** [SRS-UI-001](./srs.md#srs-ui-001-library-chrome), [PRD coverage](./srs.md#prd-coverage) (Library and Collections sidebar groups; the remaining views have no dedicated surface yet).

**Tests:** [`automatic_and_activity_galleries_F-007.feature`](../tests/features/automatic_and_activity_galleries_F-007.feature).

### Acceptance Criteria

ImageSquid's own view criteria (moved from FileSquid F-007 on 2026-10-16). FileSquid F-007 now states only the platform capability these views call and FileSquid's default UI.

- Collections MUST include Imports, Recently Added/Saved, Edited, Viewed and Shared, favorites and received/shared-with-me media, with controls for activity retention and visibility. Apple Messages ingestion is an adapter-dependent equivalent, not presumed private API access.
- Media-type views MUST cover videos, selfies, motion/Live, portraits, panoramas, bursts, screenshots, screen recordings, slow motion, time lapse, Cinematic, animated, RAW and spatial media when identified.
- Recent Days, Months, Years, Trips, People/Pets and Groups MUST offer applicable curated highlights and All views without discarding omitted assets.
- Document, receipt, handwriting, illustration and QR collections MUST expose classification corrections and analysis progress; empty/pending collections MUST be distinguishable.

## F-008: Deletion, hidden media and duplicate review

**FileSquid requirement:** FileSquid PRD F-008, same ID. FileSquid implements the capability behind the FileSquid API; ImageSquid presents it through its own views.

**Priority:** Mandatory. **User Story:** As an owner, I want private items protected and mistakes recoverable.

**Delivered by:** FileSquid.

**ImageSquid surfaces:** [PRD coverage](./srs.md#prd-coverage) (no dedicated surface yet).

**Acceptance:** ImageSquid MUST present this capability only through the FileSquid API, MUST NOT reimplement it, and MUST meet the acceptance criteria of FileSquid F-008 for every view it shows.

## F-010: Export and portable backup

**FileSquid requirement:** FileSquid PRD F-010, same ID. FileSquid implements the capability behind the FileSquid API; ImageSquid presents it through its own views.

**Priority:** Mandatory. **User Story:** As an owner, I want to use my media elsewhere and prove I can recover it.

**Delivered by:** FileSquid.

**ImageSquid surfaces:** [SRS-UI-004](./srs.md#srs-ui-004-preservation-and-migration), [SRS-UI-002](./srs.md#srs-ui-002-media-inspector).

**Acceptance:** ImageSquid MUST present this capability only through the FileSquid API, MUST NOT reimplement it, and MUST meet the acceptance criteria of FileSquid F-010 for every view it shows.

## F-011: Non-destructive editing workflow

**FileSquid requirement:** FileSquid PRD F-011, same ID. FileSquid implements the capability behind the FileSquid API; ImageSquid presents it through its own views.

**Priority:** Mandatory. **User Story:** As an editor, I want to try changes safely and reproduce them later.

**Delivered by:** ImageSquid distribution; capabilities as FileSquid modules (FileSquid U-25).

**ImageSquid surfaces:** [SRS-UI-003](./srs.md#srs-ui-003-non-destructive-studio).

**Tests:** [`nondestructive_editing_workflow_F-011.feature`](../tests/features/nondestructive_editing_workflow_F-011.feature).

### Acceptance Criteria

ImageSquid's own view criteria (moved from FileSquid F-011 on 2026-10-16). FileSquid F-011 now states only the platform capability these views call and FileSquid's default UI.

- All edits MUST preserve originals; versioned recipes MUST support undo/redo, revert, before/after, duplicate versions, per-adjustment enable/disable/reset and Auto Enhance.
- Selective copy/paste MUST apply to one/multiple items and report skipped incompatible adjustments. Crop, retouch, red-eye and external-extension settings MUST be excluded by default in the Photos-equivalent copy workflow.
- Recipe and rendering-engine versions MUST be retained; upgrades MUST not silently change an approved appearance. Old rendered results MUST remain available when old engines cannot run.
- Desktop/browser renderers MUST be checked against the same reference corpus and published tolerances; save, crash recovery and concurrent edits MUST retain recoverable versions.
- Edit/undo/revert and rendered-output commits MUST participate in F-043 action recording without mutating previous signed manifests or original resources.
- Retained edit/metadata versions and derivatives MUST also use F-043's recoverable PROV-O transformation contract; cache-only preview generation remains separate until a result is retained or disclosed (FileSquid T-037).
- Assistive standard editing that does not substantially alter the input or its semantics (crop, exposure, and equivalent) MUST NOT be classified or labelled as AI-generated content ([FE-009](./product-vision.md#non-destructive-creativity), FileSquid C-017).

## F-012: Composition and tonal/color editing

**FileSquid requirement:** FileSquid PRD F-012, same ID. FileSquid implements the capability behind the FileSquid API; ImageSquid presents it through its own views.

**Priority:** Mandatory. **User Story:** As an editor, I want complete everyday composition and exposure/color control.

**Delivered by:** ImageSquid distribution; capabilities as FileSquid modules (FileSquid U-25).

**ImageSquid surfaces:** [SRS-UI-003](./srs.md#srs-ui-003-non-destructive-studio).

**Tests:** [`composition_and_tonal_color_editing_F-012.feature`](../tests/features/composition_and_tonal_color_editing_F-012.feature).

### Acceptance Criteria

ImageSquid's own view criteria (moved from FileSquid F-012 on 2026-10-16). FileSquid F-012 now states only the platform capability these views call and FileSquid's default UI.

- Composition MUST include free/preset/custom/original-ratio crop, auto crop/straighten suggestions, rotate, horizontal/vertical flip, straighten, horizontal/vertical perspective and independent reset.
- Tone MUST include coordinated light, brilliance-equivalent balancing, exposure, highlights, shadows, brightness, contrast, black point, global/channel curves, levels and live histograms.
- Color MUST include coordinated color, saturation, vibrance, cast, white balance with gray/skin/temperature-tint modes and eyedropper.
- Selective Color MUST support at least six independent sampled colors with hue, saturation, luminance and range controls. Monochrome MUST provide intensity, neutrals, tone and grain.
- Every control MUST expose its current value, reset and accessible input; export MUST apply the saved settings consistently.

## F-013: Detail, filters, retouch and RAW

**FileSquid requirement:** FileSquid PRD F-013, same ID. FileSquid implements the capability behind the FileSquid API; ImageSquid presents it through its own views.

**Priority:** Mandatory. **User Story:** As an enthusiast, I want finishing tools and fidelity for camera originals.

**Delivered by:** ImageSquid distribution; capabilities as FileSquid modules (FileSquid U-25).

**ImageSquid surfaces:** [SRS-UI-003](./srs.md#srs-ui-003-non-destructive-studio).

**Tests:** [`detail_filters_retouch_and_raw_F-013.feature`](../tests/features/detail_filters_retouch_and_raw_F-013.feature).

### Acceptance Criteria

ImageSquid's own view criteria (moved from FileSquid F-013 on 2026-10-16). FileSquid F-013 now states only the platform capability these views call and FileSquid's default UI.

- Editing MUST provide definition/clarity, noise reduction, sharpening, vignette and adjustable-strength color/monochrome filters.
- Retouch MUST expose brush size and preserve edit history; red-eye correction MUST be available on supported stills and unavailable on video.
- Supported RAW and ProRAW MUST be preserved, developed and exported unchanged on request. RAW+JPEG pairs MUST remain related and allow an explicit editing-original choice.
- RAW engine upgrades MUST allow deliberate reprocessing and comparison without silently replacing an approved render; supported cameras and decoding limitations MUST be published.

## F-014: AI-assisted cleanup and styles

**FileSquid requirement:** FileSquid PRD F-014, same ID. FileSquid implements the capability behind the FileSquid API; ImageSquid presents it through its own views.

**Priority:** Conditional mandatory for supported models/media. **User Story:** As an editor, I want to remove distractions and apply capture-aware looks safely.

**Delivered by:** ImageSquid distribution; capabilities as FileSquid modules (FileSquid U-25).

**ImageSquid surfaces:** [SRS-UI-003](./srs.md#srs-ui-003-non-destructive-studio).

**Tests:** [`aiassisted_cleanup_and_styles_F-014.feature`](../tests/features/aiassisted_cleanup_and_styles_F-014.feature).

### Acceptance Criteria

ImageSquid's own view criteria (moved from FileSquid F-014 on 2026-10-16). FileSquid F-014 now states only the platform capability these views call and FileSquid's default UI.

- Cleanup MUST provide suggested distractions and click/brush/circle selection, preview, cancellation, reversible application and explicit generation failure states.
- The studio's Tools group MUST offer **Extend** (generate image content beyond the frame to a chosen aspect ratio or margin) and **Reframe** (model-assisted perspective change, falling back to F-012 geometric perspective correction when no supported model exists), each with preview, cancellation, reversible application and explicit generation-failure states through a supported F-045 task route. Both are retained material AI manipulations under [FE-010](./product-vision.md#non-destructive-creativity): the C2PA/PROV-O action and the machine-readable AI mark apply, and the viewer and inspector MUST label the resulting rendition AI-manipulated.
- Face obscuring MUST provide a pixelation treatment, retained as a reversible local edit; exports intended to conceal identity MUST render the selected effect and omit recoverable source layers.
- Eligible styles MUST support selection/replacement and tone/color refinement using available capture data. Generic styling MUST be labeled separately from capture-aware interpretation.
- AI outputs MUST retain input/version/model provenance and MUST NOT overwrite originals. Exact Apple model/rendering reproduction is excluded; normal operation MUST follow F-029's private-library and selected-disclosure boundaries, use F-045 for BYO models, and update F-043 for required AI/action disclosure.
- When ImageSquid, under its own name, performs a retained material AI manipulation (generative fill, person removal, face swap, or equivalent), it MUST record the action in the C2PA/PROV-O history and MUST attach a machine-readable AI-generated-or-manipulated mark to the retained derivative as far as technically feasible ([FE-010](./product-vision.md#non-destructive-creativity)).

## F-015: Portrait, depth, Cinematic and spatial media

**FileSquid requirement:** FileSquid PRD F-015, same ID. FileSquid implements the capability behind the FileSquid API; ImageSquid presents it through its own views.

**Priority:** Conditional mandatory for supported capture formats. **User Story:** As an owner of specialized captures, I want their information preserved and usable controls when supported.

**Delivered by:** ImageSquid distribution; capabilities as FileSquid modules (FileSquid U-25).

**ImageSquid surfaces:** [SRS-UI-003](./srs.md#srs-ui-003-non-destructive-studio).

**Tests:** [`portrait_depth_cinematic_and_spatial_media_F-015.feature`](../tests/features/portrait_depth_cinematic_and_spatial_media_F-015.feature).

### Acceptance Criteria

ImageSquid's own view criteria (moved from FileSquid F-015 on 2026-10-16). FileSquid F-015 now states only the platform capability these views call and FileSquid's default UI.

- Portrait MUST support effect on/off, depth blur, eligible refocus, Natural/Studio/Contour/Stage/Stage Mono/High-Key Mono-equivalent lighting and combined motion/Portrait resource handling where source data is accessible.
- Cinematic MUST support effect on/off, time-based subject/point focus, held distance, automatic/manual focus decisions, depth/foreground-background blur and audio control on declared compatible captures.
- Depth, focus, spatial and other opaque capture resources MUST be preserved even when the renderer cannot interpret them; a flattened source rendition MUST remain linked and labeled.
- Spatial viewing MUST declare hardware/output prerequisites and retain an ordinary supported viewing fallback. Per-format import/edit/export fixtures MUST demonstrate each claimed capability.

## F-016: Live Photos, bursts and motion images

**FileSquid requirement:** FileSquid PRD F-016, same ID. FileSquid implements the capability behind the FileSquid API; ImageSquid presents it through its own views.

**Priority:** Mandatory with format-specific capability gates. **User Story:** As a viewer/editor, I want paired captures to remain coherent.

**Delivered by:** ImageSquid distribution; capabilities as FileSquid modules (FileSquid U-25).

**ImageSquid surfaces:** [SRS-UI-003](./srs.md#srs-ui-003-non-destructive-studio).

**Tests:** [`live_photos_bursts_and_motion_images_F-016.feature`](../tests/features/live_photos_bursts_and_motion_images_F-016.feature).

### Acceptance Criteria

ImageSquid's own view criteria (moved from FileSquid F-016 on 2026-10-16). FileSquid F-016 now states only the platform capability these views call and FileSquid's default UI.

- Still/motion resources MUST remain one logical capture with independent checksums; playback MUST expose motion/sound toggles, key frame and trim controls.
- Supported captures MUST provide Loop, Bounce and Long Exposure equivalents, applicable photo adjustments, Live/still duplication, eligible animated GIF export and revert.
- Burst review MUST show the sequence, permit comparison and select retained frames; discards MUST use recoverable deletion and preserve source relationships.
- Missing motion components MUST produce a partial-pair state rather than claim a complete Live Photo.

## F-017: Video and audio editing

**FileSquid requirement:** FileSquid PRD F-017, same ID. FileSquid implements the capability behind the FileSquid API; ImageSquid presents it through its own views.

**Priority:** Mandatory with codec/capture gates. **User Story:** As a video owner, I want ordinary video finishing without losing recorded material.

**Delivered by:** ImageSquid distribution; capabilities as FileSquid modules (FileSquid U-25).

**ImageSquid surfaces:** [SRS-UI-003](./srs.md#srs-ui-003-non-destructive-studio).

**Tests:** [`video_and_audio_editing_F-017.feature`](../tests/features/video_and_audio_editing_F-017.feature).

### Acceptance Criteria

ImageSquid's own view criteria (moved from FileSquid F-017 on 2026-10-16). FileSquid F-017 now states only the platform capability these views call and FileSquid's default UI.

- Video MUST offer non-destructive trim, poster frame, Auto Enhance, supported whole-clip tone/color/curves/levels/detail, filters, crop/rotate/flip/straighten/perspective, still-frame extraction and full restoration.
- Supported paths MUST preserve/play/export 10-bit HDR, edit slow-motion ranges and eligible high-frame-rate speed. Output MUST disclose resolution, frame rate, bit depth, color transfer and any conversion.
- Audio MUST support mute and compatible enhancement. Audio Mix equivalents MUST offer original/Standard, in-frame emphasis, studio noise/reverberation reduction, cinematic dialogue/environment balance and intensity when required audio resources are available.
- Video MUST NOT expose unsupported still-only retouch, red-eye or photo Markup controls as functioning video tools.

## F-018: Markup and companion annotation

**FileSquid requirement:** FileSquid PRD F-018, same ID. FileSquid implements the capability behind the FileSquid API; ImageSquid presents it through its own views.

**Priority:** Mandatory; device-specific input is conditional. **User Story:** As a communicator, I want to annotate a photograph accessibly.

**Delivered by:** ImageSquid distribution; capabilities as FileSquid modules (FileSquid U-25).

**ImageSquid surfaces:** [SRS-UI-003](./srs.md#srs-ui-003-non-destructive-studio).

**Tests:** [`markup_and_companion_annotation_F-018.feature`](../tests/features/markup_and_companion_annotation_F-018.feature).

### Acceptance Criteria

ImageSquid's own view criteria (moved from FileSquid F-018 on 2026-10-16). FileSquid F-018 now states only the platform capability these views call and FileSquid's default UI.

- Still-image markup MUST include freehand sketch with user-toggleable shape recognition, shapes/arrows, text/font/style/color, line/fill/shadow, highlights, magnifier, stickers and crop/orientation.
- Layers/objects within the annotation operation MUST remain editable as a recipe; this does not require a general professional layer compositor.
- Pressure-sensitive input MUST be used where available with a standard-input alternative. A companion workflow MUST pair authorized devices and return edits without exposing the library to unrelated devices.
- Accessibility descriptions MUST be editable independently of visible image text and included only according to export/sharing policy.

## F-019: People, pets and groups

**FileSquid requirement:** FileSquid PRD F-019, same ID. FileSquid implements the capability behind the FileSquid API; ImageSquid presents it through its own views.

**Priority:** Mandatory. **User Story:** As a curator, I want to find people/pets and correct recognition mistakes.

**Delivered by:** ImageSquid distribution; capabilities as FileSquid modules (FileSquid U-25).

**ImageSquid surfaces:** [PRD coverage](./srs.md#prd-coverage) (no dedicated surface yet).

**Tests:** [`people_pets_and_groups_F-019.feature`](../tests/features/people_pets_and_groups_F-019.feature).

### Acceptance Criteria

ImageSquid's own view criteria (moved from FileSquid F-019 on 2026-10-16). FileSquid F-019 now states only the platform capability these views call and FileSquid's default UI.

- Analysis MUST detect/group supported faces in stills/video and supported dogs/cats, assign multiple identities per item and expose processing status.
- Users MUST name/rename, manually add regions, confirm/reject matches, mark incorrect identities, merge duplicates and propagate accepted corrections without silently reinstating rejected assignments.
- Key portraits, favorite identities, alphabetic/manual order and display hiding MUST be available. Identity hiding and memory suppression MUST be distinguishable from deleting media.
- Automatic co-occurrence groups and custom groups MUST support membership edits, names/order, highlights/All and organizational deletion.
- Recognition enable/disable/reset/rebuild MUST be explicit; protected identity information MUST synchronize only under the encryption and permission model. Compatible model/task choice and durable corrections MUST follow F-045; general chat capabilities do not replace the dedicated recognition contract.
- Household people/pets organisation MUST remain available without a deployer-style notice ([FE-013](./product-vision.md#private-discovery), [ImageSquid U-04](./product-vision.md#user-requirements)).
- When people or biometric categorisation analysis is used under professional authority, ImageSquid MUST support informing natural persons exposed to that operation ([FE-014](./product-vision.md#private-discovery)).
- Emotion recognition, untargeted facial-image scraping and remote biometric identification in publicly accessible spaces MUST NOT be implemented ([FE-015](./product-vision.md#private-discovery), FileSquid C-020). Notice MUST NOT be claimed as GDPR compliance ([FE-016](./product-vision.md#private-discovery)).
- The GDPR and biometric-analysis review named in the ImageSquid vision's standards table MUST be completed and recorded before the ImageSquid release. F-019 MUST NOT pass release acceptance, and recognition MUST NOT ship enabled, without that recorded review ([FE-016](./product-vision.md#private-discovery)).

## F-020: Semantic search, OCR and visual understanding

**FileSquid requirement:** FileSquid PRD F-020, same ID. FileSquid implements the capability behind the FileSquid API; ImageSquid presents it through its own views.

**Priority:** Mandatory with model/language gates. **User Story:** As a viewer, I want to find media by its content and use recognized information.

**Delivered by:** ImageSquid distribution; capabilities as FileSquid modules (FileSquid U-25).

**ImageSquid surfaces:** [SRS-UI-001](./srs.md#srs-ui-001-library-chrome), [PRD coverage](./srs.md#prd-coverage) (search field; results, OCR and visual-understanding views have no dedicated surface yet).

**Tests:** [`semantic_search_ocr_and_visual_understanding_F-020.feature`](../tests/features/semantic_search_ocr_and_visual_understanding_F-020.feature).

### Acceptance Criteria

ImageSquid's own view criteria (moved from FileSquid F-020 on 2026-10-16). FileSquid F-020 now states only the platform capability these views call and FileSquid's default UI.

- Search MUST cover objects/scenes/activities, people/pets, location/date combinations, OCR, natural-language queries and relevant time-coded video moments; contributor search MUST respect shared-library permissions.
- OCR MUST support selection/copy, translation for declared languages, dictionary lookup, web search, text sharing and phone call/message, email composition/contact creation, and URL preview/open actions. External actions MUST require a user action and disclose any text sent outside the device.
- Visual lookup MUST support declared landmarks/artwork/plants/flowers/animals/objects and copyable contextual information with provenance; landmark matching without GPS MUST have its own enable/disable control.
- Subject extraction MUST produce a previewable cutout that can be copied/shared/exported; corrections and unavailable models MUST be visible.
- Before recognition/search release acceptance, a consented labeled corpus MUST specify languages, people/pet classes, scenes, video moments, relevant-result judgments and approved precision/recall or task-success/error thresholds; publish performance by supported class rather than averaging away unsupported groups.
- Search/index rebuild, model changes and erroneous classifications MUST be reviewable. No recognition/lookup result MUST be represented as certain identification when it is an inference. Connected model selection and external disclosures MUST follow F-045.

## F-021: Places, Trips and curation controls

**FileSquid requirement:** FileSquid PRD F-021, same ID. FileSquid implements the capability behind the FileSquid API; ImageSquid presents it through its own views.

**Priority:** Mandatory. **User Story:** As a traveler, I want geographic and chronological discovery under my control.

**Delivered by:** ImageSquid distribution; capabilities as FileSquid modules (FileSquid U-25).

**ImageSquid surfaces:** [SRS-UI-001](./srs.md#srs-ui-001-library-chrome), [PRD coverage](./srs.md#prd-coverage) (Places in the Collections sidebar group; Trips and curation views have no dedicated surface yet).

**Tests:** [`places_trips_and_curation_controls_F-021.feature`](../tests/features/places_trips_and_curation_controls_F-021.feature).

### Acceptance Criteria

ImageSquid's own view criteria (moved from FileSquid F-021 on 2026-10-16). FileSquid F-021 now states only the platform capability these views call and FileSquid's default UI.

- Map browsing and geographic search MUST use recorded or explicitly inferred locations, visibly distinguish them, and respect unknown dates/time zones.
- Trips and curated time groups MUST retain underlying assets, offer highlights/All, and allow correction of grouping/classification.
- Map/lookup services MUST disclose what coordinates or queries leave the device; unavailable network maps MUST not prevent offline metadata use.
- Analysis progress, opt-out and reindexing MUST apply to generated collections without deleting originals or manually entered metadata.

## F-022: Memories and storytelling

**FileSquid requirement:** FileSquid PRD F-022, same ID. FileSquid implements the capability behind the FileSquid API; ImageSquid presents it through its own views.

**Priority:** Mandatory with generation capability gates. **User Story:** As a family member, I want editable stories from my collection.

**Delivered by:** ImageSquid distribution; capabilities as FileSquid modules (FileSquid U-25).

**ImageSquid surfaces:** [PRD coverage](./srs.md#prd-coverage) (no dedicated surface yet).

**Tests:** [`memories_and_storytelling_F-022.feature`](../tests/features/memories_and_storytelling_F-022.feature).

### Acceptance Criteria

ImageSquid's own view criteria (moved from FileSquid F-022 on 2026-10-16). FileSquid F-022 now states only the platform capability these views call and FileSquid's default UI.

- Memories MUST include automatic themed collections, playable movies, grid inspection, pinning, source-item/presentation sharing and prompt-based creation through supported F-045 task routes with owner authorization.
- Users MUST edit title/subtitle, cover, duration, add/remove/reorder content, choose a consistent look and licensed/local soundtrack, and export supported presentation proportions.
- Users MUST create a memory from an album, a gallery or the current selection, mark a memory as a favorite from its tile or during playback, and switch the Memories view between all memories and those the user created. A memory favorite is memory state and MUST NOT favorite its source files (FileSquid F-022).
- Controls to disable featured-content generation MUST cover people/pets, places, themes/holidays; recommendation reset, featured-content settings and notification controls MUST be available.
- Memory deletion MUST retain source assets; excluded/private/hidden content MUST not be reintroduced by generation. Music rights MUST govern export; Apple Music access is not presumed.
- AI systems that interact directly with a person (including a collection assistant and prompt-assisted memories driven by a model) MUST inform that person they are interacting with AI at the start of the first interaction, in a clear, distinguishable and accessible way, unless it is obvious to a reasonably well-informed, observant person in that context ([FE-025](./product-vision.md#enjoyment-and-presentation), FileSquid U-82).
- Memories MUST NOT add emotion recognition ([FE-026](./product-vision.md#enjoyment-and-presentation)).

## F-023: Slideshows, printing and photo projects

**FileSquid requirement:** FileSquid PRD F-023, same ID. FileSquid implements the capability behind the FileSquid API; ImageSquid presents it through its own views.

**Priority:** Mandatory; third-party ordering is conditional on an authorized provider. **User Story:** As an owner, I want to present and print collections without rebuilding them elsewhere.

**Delivered by:** ImageSquid distribution; capabilities as FileSquid modules (FileSquid U-25).

**ImageSquid surfaces:** [PRD coverage](./srs.md#prd-coverage) (no dedicated surface yet).

**Tests:** [`slideshows_printing_and_photo_projects_F-023.feature`](../tests/features/slideshows_printing_and_photo_projects_F-023.feature).

### Acceptance Criteria

ImageSquid's own view criteria (moved from FileSquid F-023 on 2026-10-16). FileSquid F-023 now states only the platform capability these views call and FileSquid's default UI.

- Instant and saved slideshows MUST support themes, sequence editing, text, soundtracks, slide/presentation timing, transitions, preview, loop and video export.
- Printing MUST support single/multiple photos, standard/custom sizes, contact sheets, multi-photo layouts, printer settings, color/monochrome where supported and portable PDF output.
- Project adapters MUST support installed-provider management, create/reopen/revise supported books/cards/calendars/framed prints and print ordering with explicit destination, cost and submission confirmation.
- Legacy project conversion MUST report unsupported layouts/assets and preserve source files; it MUST NOT claim lossless reconstruction without a validated mapping.
- Slideshow playback MUST prepare upcoming and adjacent slides using bounded prefetch and suitable display derivatives, preserving current sequence and edit versions. Jumps, reversals and settings changes MUST cancel stale preparation. Missing/failed slides MUST expose their state without silent skipping; warm prepared-slide timing, cold readiness and transition lateness MUST be measured independently under NFR-002.

## F-024: System sharing and extension interfaces

**FileSquid requirement:** FileSquid PRD F-024, same ID. FileSquid implements the capability behind the FileSquid API; ImageSquid presents it through its own views.

**Priority:** Mandatory with platform-specific equivalents. **User Story:** As an owner, I want media to work with the applications and devices I use.

**Delivered by:** FileSquid (sharing and extension contracts through the API). The ImageSquid distribution adds the photo-specific surfaces (photo picker, wallpaper, external photo editors); capabilities as FileSquid modules (FileSquid U-25).

**ImageSquid surfaces:** [SRS-UI-001](./srs.md#srs-ui-001-library-chrome), [SRS-UI-002](./srs.md#srs-ui-002-media-inspector) (share controls).

**Acceptance:** ImageSquid MUST present this capability only through the FileSquid API, MUST NOT reimplement it, and MUST meet the acceptance criteria of FileSquid F-024 for every view it shows.

## F-025: Migration selection, inventory and pilot

**FileSquid requirement:** FileSquid PRD F-025, same ID. FileSquid implements the capability behind the FileSquid API; ImageSquid presents it through its own views.

**Priority:** Mandatory. **User Story:** As a migrating owner, I want a realistic preservation plan before transferring my collection.

**Delivered by:** FileSquid (migration module, FileSquid FE-029, FileSquid FE-030). Photo-platform routes are presented by the ImageSquid distribution ([F-028](#f-028-supported-migration-routes)); capabilities as FileSquid modules (FileSquid U-25).

**ImageSquid surfaces:** [SRS-UI-004](./srs.md#srs-ui-004-preservation-and-migration).

**Acceptance:** ImageSquid MUST present this capability only through the FileSquid API, MUST NOT reimplement it, and MUST meet the acceptance criteria of FileSquid F-025 for every view it shows.

## F-027: Migration execution, recovery and reconciliation

**FileSquid requirement:** FileSquid PRD F-027, same ID. FileSquid implements the capability behind the FileSquid API; ImageSquid presents it through its own views.

**Priority:** Mandatory. **User Story:** As a migrating owner, I want interruptions to preserve verified progress.

**Delivered by:** FileSquid (migration module, FileSquid FE-029, FileSquid FE-030). Photo-platform routes are presented by the ImageSquid distribution ([F-028](#f-028-supported-migration-routes)); capabilities as FileSquid modules (FileSquid U-25).

**ImageSquid surfaces:** [SRS-UI-004](./srs.md#srs-ui-004-preservation-and-migration).

**Acceptance:** ImageSquid MUST present this capability only through the FileSquid API, MUST NOT reimplement it, and MUST meet the acceptance criteria of FileSquid F-027 for every view it shows.

## F-028: Supported migration routes

**FileSquid requirement:** FileSquid PRD F-028, same ID. FileSquid implements the capability behind the FileSquid API; ImageSquid presents it through its own views.

**Priority:** Mandatory baseline routes; optional conveniences explicitly identified. **User Story:** As an ecosystem user, I want a supported route that matches where my media actually resides.

**Delivered by:** ImageSquid distribution; capabilities as FileSquid modules (FileSquid U-25).

**ImageSquid surfaces:** [SRS-UI-004](./srs.md#srs-ui-004-preservation-and-migration), [SRS-UI-008](./srs.md#srs-ui-008-unmatched-v2-controls).

**Tests:** [`supported_migration_routes_F-028.feature`](../tests/features/supported_migration_routes_F-028.feature).

### Acceptance Criteria

ImageSquid's own view criteria (moved from FileSquid F-028 on 2026-10-16). FileSquid F-028 now states only the platform capability these views call and FileSquid's default UI.

- SRC-01 Apple export folders MUST accept originals, XMP and separate edited renders without assuming flat exports include albums/faces. SRC-02 macOS PhotoKit MUST inventory only authorized accessible libraries/resources, request cloud originals in bounded batches and guarantee completed-resource recovery; arbitrary libraries and byte-offset extraction MUST not be assumed.
- SRC-03/04 Google Takeout archives/extracted folders MUST support multiple parts, late JSON sidecars, filename collisions, duplicate content and cross-part albums; download time MUST not replace capture time.
- SRC-07 Windows folders and SRC-08 hydrated OneDrive folders MUST identify local/OneDrive/iCloud origins, detect placeholders, preserve folders/embedded metadata, report application-only metadata gaps, avoid cross-route duplicates, and copy to a destination outside provider synchronization.
- SRC-05 Google Picker selected imports and SRC-09 Graph/OneDrive direct downloads MAY be added; neither makes selected import a whole-library Google route. SRC-06 provider-to-provider transfer MAY be researched only after eligibility and preservation exclusions are verified.
- Each route MUST publish supported source/export/OS versions, permissions and preservation matrix. Private Apple database reading MAY be an isolated read-only experiment; it is not the initial supported foundation.
- ImageSquid MUST NOT claim transfer of Apple adjustment history, source sharing permissions, faces or dynamic memories unless a route is implemented and tested. Automatic source deletion MUST NOT be included ([C-005](./product-vision.md#platforms-originals-and-service-limits)).
- Where a source lets one item belong to many albums, the route MUST import files into one album per import and MUST turn every source album into a hand-picked gallery preserving source order and cover, so that no source membership is dropped and the owner is not asked to choose a primary album (FileSquid F-026, [F-004](#f-004-albums-folders-and-navigation)).
- Every inventoried item MUST receive an outcome, a preservation explanation and a recovery path (FileSquid FE-029). Migration completion and feature parity MUST remain separate claims ([C-039](./product-vision.md#parity-preservation-and-sequence)).
- Safe migration MUST remain a reusable capability whose placement is governed by its dependencies (FileSquid C-041). Native Apple acquisition MUST remain a macOS adapter rather than a Linux prerequisite ([FE-018](./product-vision.md#cross-platform-access)).
- Broad Google Library API reading and Google Data Portability Photos coverage MUST NOT be assumed; direct connectors MUST revalidate official contracts before implementation.

## F-033: Shared albums and hosted galleries

**FileSquid requirement:** FileSquid PRD F-033, same ID. FileSquid implements the capability behind the FileSquid API; ImageSquid presents it through its own views.

**Priority:** Mandatory at maturity; hosted operation may be paid. **User Story:** As an owner, I want selected audiences to view or contribute to an album.

**Delivered by:** FileSquid.

**ImageSquid surfaces:** [PRD coverage](./srs.md#prd-coverage) (no dedicated surface yet).

**Acceptance:** ImageSquid MUST present this capability only through the FileSquid API, MUST NOT reimplement it, and MUST meet the acceptance criteria of FileSquid F-033 for every view it shows.

## F-034: Family accounts and shared libraries

**FileSquid requirement:** FileSquid PRD F-034, same ID. FileSquid implements the capability behind the FileSquid API; ImageSquid presents it through its own views.

**Priority:** Mandatory family and non-family group outcome; operated packaging conditional on the offered service. **User Story:** As a family or group member, I want collaboration while keeping personal media private.

**Delivered by:** FileSquid.

**ImageSquid surfaces:** [SRS-UI-001](./srs.md#srs-ui-001-library-chrome), [PRD coverage](./srs.md#prd-coverage) (current-library switcher).

**Acceptance:** ImageSquid MUST present this capability only through the FileSquid API, MUST NOT reimplement it, and MUST meet the acceptance criteria of FileSquid F-034 for every view it shows.

## F-036: Browser and mobile-web experience

**FileSquid requirement:** FileSquid PRD F-036, same ID. FileSquid implements the capability behind the FileSquid API; ImageSquid presents it through its own views.

**Priority:** Mandatory. **User Story:** As a user away from my desktop, I want useful access with clear limitations.

**Delivered by:** FileSquid (browser hosts, PWA install and selected upload through the API). The ImageSquid distribution presents the phone photo sync and in-app camera views below (FileSquid U-25).

**ImageSquid surfaces:** [SRS-UI-006](./srs.md#srs-ui-006-mobile-web-companion), [IS-S-011](./srs.md#mobile-photo-sync-and-capture)–[IS-S-021](./srs.md#mobile-photo-sync-and-capture).

**Tests:** [`mobile_companion_SRS-UI-006.feature`](../tests/features/mobile_companion_SRS-UI-006.feature).

### Acceptance Criteria

ImageSquid's own view criteria, added 2026-10-16 from the owner's mobile PWA photo sync requirements (FR-01–FR-11). Most ImageSquid users are expected to use the installed PWA on an iPhone or Android phone, so these views are ImageSquid's primary mobile surface. ImageSquid MUST also meet FileSquid F-036 and, for every photo it ingests, the FileSquid F-002 import workflow and F-043 import record. Ingestion means a verified upload through `FileSquidClient` to the connected host; the PWA keeps no library of its own.

**Photo sync**

- The main screen MUST provide a **Sync Now** control that starts photo ingestion when the user activates it (FR-01).
- When the app opens, it MUST attempt to sync new photos without user action where the platform permits: a remembered folder whose access is still granted. Where the platform requires a user gesture, opening the app MUST NOT start a picker or permission prompt; it MUST offer the one-tap path instead (FR-02).
- Photo access MUST use the File System Access API where the browser provides it (expected: Android and desktop browsers) and the file input picker where it does not (expected: iOS) (FR-03). The choice MUST come from runtime capability detection, not from the user agent (FR-10).
- Where the File System Access API is available, the app MUST ask the user to choose a photo folder once and MUST request persistent access to it, so later syncs do not ask for the folder again. When the browser revokes or downgrades that access, the app MUST say so and offer one-tap re-grant or re-selection (FR-04).
- Each successful sync MUST record its time. Later syncs of a remembered folder MUST ingest only photos created or modified after that time (FR-05). A photo already in the library MUST NOT be ingested twice, whatever its timestamp; FileSquid F-002 previously-imported detection decides. A lost sync record MUST cause a rescan that skips already-imported photos, never duplicates.
- The main screen MUST always show the date and time of the last successful sync, or that no sync has completed yet (FR-06). Photo sync status MUST be visually and textually distinct from the **Archives** backup status.
- After detecting new photos, the app MUST show how many are pending before or during sync (for example "14 new photos found"), then progress and per-photo failures (FR-07).
- On iOS, where persistent folder access is unavailable, the app MUST tell the user that photos are selected each sync session, MUST open the picker directly from **Sync Now** with multiple selection and photo media types preset, and MUST skip photos already in the library without asking (FR-08).

**Camera capture**

- The app MUST let the user take a photo with the device camera from within the app and MUST add the captured photo to the user's library without further steps (FR-09). Until its upload is verified, the captured photo MUST show as not yet in the library; it MUST NOT be presented as protected while it exists only in browser storage.

**Platform capability**

- The app MUST detect at runtime which photo access capabilities the host browser provides (directory access, persistent permission, multi-file picker, camera capture) and MUST adapt its sync controls and behaviour to them (FR-10).
- Where the advanced photo access APIs are unavailable, the app MUST fall back to the manual file picker and MUST NOT show sync options the platform does not support (FR-11).
- Sync runs only while the app is open. Sync MUST NOT promise background or after-close ingestion (FileSquid C-001), and an interrupted sync MUST resume from verified photos when the app next runs.

## F-037: Setup, updates and diagnostics

**FileSquid requirement:** FileSquid PRD F-037, same ID. FileSquid implements the capability behind the FileSquid API; ImageSquid presents it through its own views.

**Priority:** Mandatory. **User Story:** As an owner/operator, I want installation and maintenance without fragile manual steps.

**Delivered by:** FileSquid.

**ImageSquid surfaces:** [SRS-UI-008](./srs.md#srs-ui-008-unmatched-v2-controls), [Distribution requirements](./srs.md#distribution-requirements) (task monitor; IS-S-009 brand assets).

**Acceptance:** ImageSquid MUST present this capability only through the FileSquid API, MUST NOT reimplement it, and MUST meet the acceptance criteria of FileSquid F-037 for every view it shows.

## F-039: Consent, notifications and activity privacy

**FileSquid requirement:** FileSquid PRD F-039, same ID. FileSquid implements the capability behind the FileSquid API; ImageSquid presents it through its own views.

**Priority:** Mandatory. **User Story:** As an owner, I want private analysis and sharing to remain intentional.

**Delivered by:** FileSquid.

**ImageSquid surfaces:** [PRD coverage](./srs.md#prd-coverage) (no dedicated surface yet).

**Acceptance:** ImageSquid MUST present this capability only through the FileSquid API, MUST NOT reimplement it, and MUST meet the acceptance criteria of FileSquid F-039 for every view it shows.

## F-040: Compatibility and parity register

**FileSquid requirement:** FileSquid PRD F-040, same ID. FileSquid implements the capability behind the FileSquid API; ImageSquid presents it through its own views.

**Priority:** Mandatory. **User Story:** As a prospective user, I want to know exactly which outcomes and formats are supported.

**Delivered by:** FileSquid (capability register for every host and distribution). The photo-manager parity register belongs to the ImageSquid distribution ([C-036](./product-vision.md#parity-preservation-and-sequence), [C-037](./product-vision.md#parity-preservation-and-sequence)).

**ImageSquid surfaces:** [PRD coverage](./srs.md#prd-coverage) (no dedicated surface yet).

**Acceptance:** ImageSquid MUST present this capability only through the FileSquid API, MUST NOT reimplement it, and MUST meet the acceptance criteria of FileSquid F-040 for every view it shows.

## F-041: Source-aware web clipping and HTTP evidence

**FileSquid requirement:** FileSquid PRD F-041, same ID. FileSquid implements the capability behind the FileSquid API; ImageSquid presents it through its own views.

**Priority:** Mandatory. **User Story:** As a source collector, I want saved images/videos to retain the complete recorded acquisition evidence so I can inspect their source and subsequent history.

**Delivered by:** FileSquid.

**ImageSquid surfaces:** [SRS-UI-002](./srs.md#srs-ui-002-media-inspector).

**Acceptance:** ImageSquid MUST present this capability only through the FileSquid API, MUST NOT reimplement it, and MUST meet the acceptance criteria of FileSquid F-041 for every view it shows.

## F-042: OCFL preservation and portable recovery

**FileSquid requirement:** FileSquid PRD F-042, same ID. FileSquid implements the capability behind the FileSquid API; ImageSquid presents it through its own views.

**Priority:** Mandatory for canonical local preservation from the first FileSquid release and for the paid protection service. **User Story:** As an owner, I want a directly accessible, complete local archive and private hosted protection that remain recoverable after software, provider or account changes.

**Delivered by:** FileSquid.

**ImageSquid surfaces:** [SRS-UI-004](./srs.md#srs-ui-004-preservation-and-migration).

**Acceptance:** ImageSquid MUST present this capability only through the FileSquid API, MUST NOT reimplement it, and MUST meet the acceptance criteria of FileSquid F-042 for every view it shows.

## F-043: Import-time C2PA records and action disclosure

**FileSquid requirement:** FileSquid PRD F-043, same ID. FileSquid implements the capability behind the FileSquid API; ImageSquid presents it through its own views.

**Priority:** Mandatory for every import and disclosure-relevant action. **User Story:** As an owner, I want provenance to begin when FileSquid receives media and follow the actions it takes.

**Delivered by:** FileSquid.

**ImageSquid surfaces:** [SRS-UI-002](./srs.md#srs-ui-002-media-inspector).

**Acceptance:** ImageSquid MUST present this capability only through the FileSquid API, MUST NOT reimplement it, and MUST meet the acceptance criteria of FileSquid F-043 for every view it shows.

## F-044: Paid C2PA publishing and watermarking

**FileSquid requirement:** FileSquid PRD F-044, same ID. FileSquid implements the capability behind the FileSquid API; ImageSquid presents it through its own views.

**Priority:** Mandatory product capability; activation is optional for the user and requires the selected paid entitlement. **User Story:** As a subscriber, I want a third-party publisher to return a C2PA-published and watermarked version of my image with verifiable provenance.

**Delivered by:** FileSquid.

**ImageSquid surfaces:** [SRS-UI-005](./srs.md#srs-ui-005-paid-c2pa-publication).

**Acceptance:** ImageSquid MUST present this capability only through the FileSquid API, MUST NOT reimplement it, and MUST meet the acceptance criteria of FileSquid F-044 for every view it shows.

## F-045: BYO AI connections and per-task model choice

**FileSquid requirement:** FileSquid PRD F-045, same ID. FileSquid implements the capability behind the FileSquid API; ImageSquid presents it through its own views.

**Priority:** Mandatory with model/task capability gates. **User Story:** As an owner, I want to use my existing local runner or provider account and choose a suitable model for each collection task.

**Delivered by:** FileSquid.

**ImageSquid surfaces:** [PRD coverage](./srs.md#prd-coverage) (no dedicated surface yet).

**Acceptance:** ImageSquid MUST present this capability only through the FileSquid API, MUST NOT reimplement it, and MUST meet the acceptance criteria of FileSquid F-045 for every view it shows.

## F-046: Media metadata inspection, correction and privacy exports

**FileSquid requirement:** FileSquid PRD F-046, same ID. FileSquid implements the capability behind the FileSquid API; ImageSquid presents it through its own views.

**Priority:** Mandatory, free core capability. Scrubbing is user-configurable; every sharing/publication route MUST honor the selected policy. **User Story:** As an owner, I want Dublin Core descriptive metadata and EXIF technical metadata in my private collection, and control over which fields remain when I share or publish.

**Delivered by:** FileSquid.

**ImageSquid surfaces:** [SRS-UI-002](./srs.md#srs-ui-002-media-inspector).

**Acceptance:** ImageSquid MUST present this capability only through the FileSquid API, MUST NOT reimplement it, and MUST meet the acceptance criteria of FileSquid F-046 for every view it shows.

## F-047: Institutional DAM and IIIF interoperability

**FileSquid requirement:** FileSquid PRD F-047, same ID. FileSquid implements the capability behind the FileSquid API; ImageSquid presents it through its own views.

**Priority:** Mandatory later institutional product; independently gated from consumer releases and photo-manager parity. **User Story:** As a GLAM collection manager, I want to curate, preserve and publish selected institutional media through declared professional metadata and collection-system interfaces without losing control of originals or private evidence.

**Delivered by:** FileSquid modules (FileSquid FE-063, FileSquid FE-079). The Arkrim distribution is the primary UI; the ImageSquid distribution MAY expose the photo-collection subset.

**ImageSquid surfaces:** [PRD coverage](./srs.md#prd-coverage) (no dedicated surface yet).

**Acceptance:** ImageSquid MUST present this capability only through the FileSquid API, MUST NOT reimplement it, and MUST meet the acceptance criteria of FileSquid F-047 for every view it shows.

## F-049: Product component terminology and item type

**FileSquid requirement:** FileSquid PRD F-049, same ID. FileSquid implements the capability behind the FileSquid API; ImageSquid presents it through its own views.

**Priority:** Mandatory. **User Story:** As an owner, I want FileSquid and every distribution to share one internal vocabulary, while the ImageSquid interface still talks like a photo app.

**Delivered by:** FileSquid.

**ImageSquid surfaces:** [SRS-UI-001](./srs.md#srs-ui-001-library-chrome).

**Acceptance:** ImageSquid MUST present this capability only through the FileSquid API, MUST NOT reimplement it, and MUST meet the acceptance criteria of FileSquid F-049 for every view it shows.

## F-050: FileSquid platform and distributions

**FileSquid requirement:** FileSquid PRD F-050, same ID. FileSquid implements the capability behind the FileSquid API; ImageSquid presents it through its own views.

**Priority:** Mandatory. **User Story:** As an owner, I want one platform that preserves and retrieves any file type, with a user interface made for my audience on top of it.

**Delivered by:** FileSquid.

**ImageSquid surfaces:** [Distribution requirements](./srs.md#distribution-requirements), [PRD coverage](./srs.md#prd-coverage) (IS-S-001).

**Acceptance:** ImageSquid MUST present this capability only through the FileSquid API, MUST NOT reimplement it, and MUST meet the acceptance criteria of FileSquid F-050 for every view it shows.

## F-055: Default UI and panel composition

**FileSquid requirement:** FileSquid PRD F-055, same ID. FileSquid implements the capability behind the FileSquid API; ImageSquid presents it through its own views.

**Priority:** Mandatory for the FileSquid default distribution and for every distribution that shows file facts. **User Story:** As a user, I want to browse and manage my files like a familiar cloud drive, with metadata and credential panels beside each file; as an ImageSquid owner, I want the inspector to show the same trusted panels.

**Delivered by:** FileSquid (`@filesquid/ui`). The ImageSquid distribution composes the panels in its inspector.

**ImageSquid surfaces:** [SRS-UI-002](./srs.md#srs-ui-002-media-inspector).

**Acceptance:** ImageSquid MUST present this capability only through the FileSquid API, MUST NOT reimplement it, and MUST meet the acceptance criteria of FileSquid F-055 for every view it shows.

## F-058: White-label distributions

**FileSquid requirement:** FileSquid PRD F-058, same ID. FileSquid implements the capability behind the FileSquid API; ImageSquid presents it through its own views.

**Priority:** Mandatory. **User Story:** As a distribution builder, I want to ship a branded product for my audience without writing backend code.

**Delivered by:** FileSquid. The FileSquid default distribution, ImageSquid and later Arkrim are its first distributions.

**ImageSquid surfaces:** [SRS-UI-001](./srs.md#srs-ui-001-library-chrome).

**Acceptance:** ImageSquid MUST present this capability only through the FileSquid API, MUST NOT reimplement it, and MUST meet the acceptance criteria of FileSquid F-058 for every view it shows.

# Non-Functional Requirements

## NFR-002: Benchmark envelope and responsiveness

**FileSquid requirement:** FileSquid PRD NFR-002, same ID. FileSquid implements the capability behind the FileSquid API; ImageSquid presents it through its own views.

Copied from FileSquid NFR-002 on 2026-10-16. ImageSquid MUST meet it for its views and workflows; FileSquid owns the platform-wide evidence.

**ImageSquid surfaces:** [SRS-UI-001](./srs.md#srs-ui-001-library-chrome), [SRS-UI-002](./srs.md#srs-ui-002-media-inspector), [PRD coverage](./srs.md#prd-coverage).

**Requirement:** High performance MUST be a defining free requirement from the first FileSquid release, on every host and distribution, maintained as workflows expand. Proposed test profiles are Small: 10,000 assets/20,000 resources/100 GB/100 albums; Medium: 100,000/200,000/1 TB/5,000 albums; Large: 1,000,000/2,000,000/10 TB/50,000 albums/10,000,000 memberships. Include duplicate bytes, pairs, Unicode, dense memberships, late sidecars, high decoded-pixel counts and varied supported formats. Support tests MUST include a 100 GB single resource and 250 GB archive part through streaming rather than memory-sized allocation; compressed bytes alone MUST NOT establish bounded decoding cost.

On a declared 4-core/16 GB/SSD desktop reference machine, warm viewport interaction-to-visible-feedback p95 MUST be ≤100 ms, indexed metadata search p95 ≤2 s, and first useful local viewport ≤3 s for the large catalog. Measure at least 100 queries/interactions after declared warm-up and report cold behavior separately. Mobile/browser tests MUST use a separately declared 4 GB device/network profile: warm cached control feedback p95 MUST be ≤100 ms, while completed common browse/search work retains its p95 ≤2 s budget excluding unavailable-original downloads. Excluded transfer waits MUST still appear in end-to-end reports.

For each supported desktop/browser/mobile profile, resident prepared thumbnail/viewer images and prepared next slides MUST paint the correct image at p95 ≤100 ms from the request or scheduled presentation time. A placeholder, stale image or input spinner MUST NOT pass. Preparation coverage across the exercised collection MUST be reported so a tiny prewarmed subset cannot establish performance. Input acknowledgment, first useful preview and final requested-quality readiness MUST have independent measurements and acceptance results.

Declared 60 Hz scrolling/presentation MUST use a proposed 16.7 ms frame budget. Before release acceptance, each profile MUST define permitted missed-frame rate, slideshow transition lateness, sufficiently large p95/p99 sample sizes, clock boundaries, warm-up/cache contents and error limits; other refresh rates require their own explicit budget. Reports MUST show stalls and p95/p99 timing rather than averages alone.

Before an affected feature ships, its nonempty format/device/local-or-remote matrix MUST define useful-preview and full-quality deadlines for cold, warm, evicted and missing-derivative cases, including supported large/high-resolution media. The first useful viewport, warm feedback and full-quality gates MUST pass independently; a fast spinner MUST NOT compensate for missed image-readiness targets. Tests MUST exercise idle operation and concurrent import/migration, thumbnail generation, indexing, provenance scans and batch metadata work, including navigation reversals/jumps, cache loss and resource pressure. Actual end-to-end wait, transfer/decode components, failures and unsupported coverage MUST remain visible; demanding cases MUST NOT disappear from evidence by labeling all relevant formats unsupported.

## NFR-003: Bounded processing and migration endurance

**FileSquid requirement:** FileSquid PRD NFR-003, same ID. FileSquid implements the capability behind the FileSquid API; ImageSquid presents it through its own views.

Copied from FileSquid NFR-003 on 2026-10-16. ImageSquid MUST meet it for its views and workflows; FileSquid owns the platform-wide evidence.

**ImageSquid surfaces:** [SRS-UI-004](./srs.md#srs-ui-004-preservation-and-migration), [PRD coverage](./srs.md#prd-coverage).

**Requirement:** Proposed desktop default staging cap is 10 GiB, with reserve max(2 GiB, 5% destination capacity) and preflight selection of a larger cap or segmented path when a resource cannot fit. Streamed large resources MUST not require a full extra library copy. Default active transfers ≤4 and CPU-heavy workers ≤2; all queues MUST have explicit finite limits/backpressure. Allow at most 5 total attempts per resource, including the initial attempt, before quarantine, using exponential retry backoff capped at 60 s plus jitter; manual retry starts a new recorded recovery cycle subject to the same cap.

Proposed migration process memory budget is ≤2 GiB excluding separately budgeted ML models. Restart MUST reach a resumable job state within 30 s on the reference desktop without rescanning completed media; measure repeated bytes per route and allow documented affected-resource/decompression replay. A 72-hour mixed/fault soak MUST show no unbounded growth; post-warm-up non-cache memory growth MUST be ≤10%. Throughput MUST be reported as verified bytes/time with hardware/network, small-file mix and sidecar reconstruction time; do not promise WAN speed independent of the provider.

Heavy decode, transform, index, scan, verification and transfer work MUST run outside the interactive rendering path through bounded CPU/GPU/I/O/transfer queues. Current controls/visible media MUST have priority, equivalent work MUST be deduplicated and speculative work MUST reduce under memory, disk, network, battery or thermal pressure. Existing default concurrency ceilings above remain in force until a reviewed profile changes them. Each scheduler profile MUST define a measurable background-service guarantee or a visible pause policy before implementation so interaction cannot silently starve preservation indefinitely; progress, pause cause and resumption MUST be observable.

Each device/browser profile MUST define finite memory/disk cache budgets and prefetch item/byte/time limits before implementation. Thumbnail/display/decoded-media and view/query caches MUST key identity, content/recipe/metadata revision, view/sort/filter and permission scope, with deterministic invalidation, pinning and eviction. Stale work MUST be canceled or its results rejected. Application eviction MUST NOT delete authoritative originals/history or pinned offline media; browser-imposed eviction/quota loss MUST expose availability and recovery limits. Peak decoded memory, cache occupancy, pending queues, hit/miss/eviction/redecode rates, speculative bytes and background progress MUST be measured with explicit bitmap/decoder/resource release.

Durable completed-item checkpoints MUST extend to supported import/upload, thumbnail/index, provenance-scan and metadata batches. Restart MUST revalidate source/version/authorization, reuse verified completed work and resume within-resource only where supported. Necessary affected-unit replay MUST be explicit and measured; faults MUST NOT cause entire-batch restart, duplicate provenance/events or false-complete media. Cache loss may require rebuilding missing cache work without pretending completed authoritative work was lost. Finite retry/backpressure policy and safe quarantine/recovery remain mandatory.

## NFR-006: Accessibility and usability

**FileSquid requirement:** FileSquid PRD NFR-006, same ID. FileSquid implements the capability behind the FileSquid API; ImageSquid presents it through its own views.

Copied from FileSquid NFR-006 on 2026-10-16. ImageSquid MUST meet it for its views and workflows; FileSquid owns the platform-wide evidence.

**ImageSquid surfaces:** [Distribution requirements](./srs.md#distribution-requirements), [inherited UI-S-013](./srs.md#inherited-from-filesquidui-copied-2026-10-16), [PRD coverage](./srs.md#prd-coverage) (IS-S-008).

**Requirement:** Web interfaces MUST target WCAG 2.2 AA; desktop MUST support keyboard-only tasks, native screen readers, visible focus, scalable text and high-contrast/reduced-motion preferences. Drag-only workflows MUST have alternatives. Release testing MUST include import, search, edit, permission review, recovery and export on each supported platform; built UI MUST mount with rigorous DOM/state assertions and no SSR/hydration/browser errors.

# Technical Constraints

## TC-003: Platform and provider limits

**FileSquid requirement:** FileSquid PRD TC-003, same ID. FileSquid implements the capability behind the FileSquid API; ImageSquid presents it through its own views.

Copied from FileSquid TC-003 on 2026-10-16. ImageSquid MUST meet it for its views and workflows; FileSquid owns the platform-wide evidence.

**ImageSquid surfaces:** [SRS-UI-004](./srs.md#srs-ui-004-preservation-and-migration), [SRS-UI-006](./srs.md#srs-ui-006-mobile-web-companion), [PRD coverage](./srs.md#prd-coverage).

**Constraint:** PhotoKit permissions/library coverage, Google export/API restrictions, Microsoft placeholders/range semantics, browser lifetime/filesystem constraints, media licensing and capture dependencies MUST be verified per supported version. Exact Apple proprietary history/rendering and undocumented APIs MUST not be assumed.

# Release Criteria

## RC-002: Performance and resource bounds

**FileSquid requirement:** FileSquid PRD RC-002, same ID. FileSquid implements the capability behind the FileSquid API; ImageSquid presents it through its own views.

Copied from FileSquid RC-002 on 2026-10-16. ImageSquid MUST meet it for its views and workflows; FileSquid owns the platform-wide evidence.

**ImageSquid surfaces:** [PRD coverage](./srs.md#prd-coverage).

**Criteria:** Assigned NFR-002/003 profiles pass from the FileSquid release and at every added workflow gate, on every host and distribution, with declared hardware/browser/codec/cache/network conditions and complete structured reports. Feedback, useful-preview/prepared-image, final-quality, frame/transition, resource and recovery thresholds MUST pass independently. Profile-specific cold-readiness, missed-frame/lateness, tail-sample and background-progress limits MUST be defined before acceptance; absent limits or an all-unsupported difficult-media profile do not pass.

**Verification:** Measure p95/p99 interaction and image readiness, frame stalls, missed-frame rate, slide lateness and preparation coverage in cold/warm/evicted/missing-cache states under idle and concurrent import/migration/thumbnail/index/scan/metadata work. Test direction changes, sort/filter/permission changes, high decoded-pixel counts, missing derivatives, browser quota loss and resource pressure. Record peak decoded/cache/process memory, staging, queue/concurrency caps, cache hit/miss/eviction/redecode, speculative bytes, background progress, resume time/completed-work reuse/repeated bytes, throughput and 72-hour soak. Relevant cloud pilots additionally load-test proposed 100 concurrently active sessions with 20 concurrent resumable transfers and disclose infrastructure sizing. This concurrency is a proposed pilot test, not a capacity promise; no measured app performance is asserted by this document.

## RC-005: Functional and accessible UI quality

**FileSquid requirement:** FileSquid PRD RC-005, same ID. FileSquid implements the capability behind the FileSquid API; ImageSquid presents it through its own views.

Copied from FileSquid RC-005 on 2026-10-16. ImageSquid MUST meet it for its views and workflows; FileSquid owns the platform-wide evidence.

**ImageSquid surfaces:** [inherited UI-S-013](./srs.md#inherited-from-filesquidui-copied-2026-10-16), [PRD coverage](./srs.md#prd-coverage).

**Criteria:** Assigned F requirements and NFR-002/006/007 pass on released platforms; no uncaught browser, hydration or SSR errors occur during built-artifact scenarios. Linux/web are initial targets; mature macOS/Windows support remains separately verified.

**Verification:** Unit/BDD/integration plus production-build UI tests with strict DOM/state assertions for the correct image/resource/edit version, useful quality, ordering, focus/selection, progressive replacement and progress/error/unavailability states. Timing/resource evidence under RC-002 MUST accompany concurrent-work browsing/slideshow cases rather than replacing DOM correctness with a responsive spinner. Include keyboard/screen-reader review and media/render golden comparisons; human review covers appearance, accessibility and permissions comprehension beyond automated assertions.

## RC-009: Source evidence and C2PA lifecycle

**FileSquid requirement:** FileSquid PRD RC-009, same ID. FileSquid implements the capability behind the FileSquid API; ImageSquid presents it through its own views.

Copied from FileSquid RC-009 on 2026-10-16. ImageSquid MUST meet it for its views and workflows; FileSquid owns the platform-wide evidence.

**ImageSquid surfaces:** [SRS-UI-002](./srs.md#srs-ui-002-media-inspector), [PRD coverage](./srs.md#prd-coverage).

**Criteria:** F-041's nonempty supported recorder matrix passes full controlled request/response capture; F-043 initializes every completed import, links prior credentials and records required C2PA actions plus recoverable PROV-O for retained transformations. Applicable free collection-wide scan/report workflows MUST pass; free/offline records and scans never depend on paid signing.

**Verification:** GET/POST/form/API/file-upload fixtures with binary bodies, repeated headers, credentials/Cookie/Set-Cookie, redirects, errors, cache/service-worker responses, raw/decoded encodings and absent/empty/unavailable/truncated components. Compare exact captured bytes and associations, require partial status for gaps, and verify protected evidence is absent from operational logs and unintended disclosures. Import camera-roll, desktop, migration and web fixtures (as their routes ship), existing credentials and unsupported embedding; inject crashes between media/record commits and check retry idempotency. Edit/metadata/AI/export/undo/redaction/watermark fixtures verify action policy, immutable prior manifests, original hashes and honest unsigned/validation states.

Verify retained metadata/`dcterms:`/`schema:`/catalog transformations and export/scrub/revert PROV-O inputs/outputs, actors/tools/time/versions survive clean recovery, while ephemeral cache work creates no unbounded permanent history. Scan mixed libraries offline and unsubscribed, interrupt/resume, change resources and compare item-level/aggregate scope, coverage, incomplete status, validation age/policy and absent/unsigned/binding/trust outcomes. Confirm scans do not sign or rewrite media and report disclosure excludes private evidence unless explicitly authorized for owner archival export.

## RC-011: Paid third-party publishing and watermark delivery

**FileSquid requirement:** FileSquid PRD RC-011, same ID. FileSquid implements the capability behind the FileSquid API; ImageSquid presents it through its own views.

Copied from FileSquid RC-011 on 2026-10-16. ImageSquid MUST meet it for its views and workflows; FileSquid owns the platform-wide evidence.

**ImageSquid surfaces:** [SRS-UI-005](./srs.md#srs-ui-005-paid-c2pa-publication), [PRD coverage](./srs.md#prd-coverage).

**Criteria:** F-044 returns a correctly watermarked derivative and bound signed credential through at least one declared provider/format/destination profile using its supported desktop/browser in-app workflow. Purchase remains optional for the owner; consumer product completion requires this working capability.

**Verification:** Selected/saved-scope authorization; original unchanged; provider-visible payload inspection; watermark-before-final-binding behavior; signature/ingredient/trust evaluation; watermark/locator recovery and declared transformation/destination tests. Inject timeout, duplicate submission, cancellation, quota, provider/lookup outage and corrupted/mismatched results; require honest state and idempotent billing. Verify retained artifacts and recovery after cancellation, separate signer/library keys, published retention/locator terms and C-7 cost/recipient evidence. A decorative logo, signature-only result or universal unsupported state fails this gate.

On every declared desktop/browser surface, verify in-app selection, price/disclosure review, submission, progress, permitted cancellation/retry, result inspection and retained-output retrieval. Provider authentication/payment handoffs MUST restore the appropriate app job/session state; an external website-only path fails the workflow requirement.

## Vision allocation

Every requirement row in the [ImageSquid vision](./product-vision.md) (U-01–U-06 and every FE- and C- row) is allocated here to at least one requirement in this PRD. FE- and C- numbers are shared with the FileSquid vision; U-numbers are per vision.

| Vision ID | PRD allocation |
|---|---|
| U-01 | [F-003](#f-003-gallery-selection-and-viewing)–[F-007](#f-007-automatic-and-activity-collections), [F-019](#f-019-people-pets-and-groups) |
| U-02 | [F-011](#f-011-non-destructive-editing-workflow)–[F-018](#f-018-markup-and-companion-annotation) |
| U-03 | [F-018](#f-018-markup-and-companion-annotation), [F-022](#f-022-memories-and-storytelling), [F-023](#f-023-slideshows-printing-and-photo-projects) |
| U-04 | [F-019](#f-019-people-pets-and-groups), [F-022](#f-022-memories-and-storytelling) |
| U-05 | [F-049](#f-049-product-component-terminology-and-item-type), [F-058](#f-058-white-label-distributions) |
| U-06 | [F-058](#f-058-white-label-distributions), [F-050](#f-050-filesquid-platform-and-distributions) |
| FE-005, FE-007 | [F-003](#f-003-gallery-selection-and-viewing), [NFR-002](#nfr-002-benchmark-envelope-and-responsiveness), [NFR-003](#nfr-003-bounded-processing-and-migration-endurance) |
| FE-006 | [F-004](#f-004-albums-folders-and-navigation), [F-005](#f-005-favorites-metadata-and-keywords), [F-006](#f-006-smart-galleries-and-metadata-search) |
| FE-008, FE-009, FE-010 | [F-011](#f-011-non-destructive-editing-workflow)–[F-014](#f-014-ai-assisted-cleanup-and-styles), [F-018](#f-018-markup-and-companion-annotation) |
| FE-012, FE-013, FE-014, FE-015, FE-016 | [F-019](#f-019-people-pets-and-groups)–[F-021](#f-021-places-trips-and-curation-controls) |
| FE-018 | [F-028](#f-028-supported-migration-routes), [F-036](#f-036-browser-and-mobile-web-experience), [F-037](#f-037-setup-updates-and-diagnostics) |
| [User convenience](./product-vision.md#user-convenience) (mobile-web use) | [F-036](#f-036-browser-and-mobile-web-experience) |
| FE-023, FE-024, FE-025, FE-026 | [F-022](#f-022-memories-and-storytelling), [F-023](#f-023-slideshows-printing-and-photo-projects) |
| FE-027 | [F-015](#f-015-portrait-depth-cinematic-and-spatial-media)–[F-017](#f-017-video-and-audio-editing), [F-024](#f-024-system-sharing-and-extension-interfaces) |
| FE-028 | [F-025](#f-025-migration-selection-inventory-and-pilot)–[F-028](#f-028-supported-migration-routes) |
| C-005, C-039 | [F-028](#f-028-supported-migration-routes) |
| C-036, C-037, C-040 | [F-040](#f-040-compatibility-and-parity-register) |
| [Terminology](./product-vision.md#terminology) | [F-049](#f-049-product-component-terminology-and-item-type), [F-058](#f-058-white-label-distributions) |
| [Product roadmap](./product-vision.md#product-roadmap) | [F-050](#f-050-filesquid-platform-and-distributions), [Release order and dependency gates](#release-order-and-dependency-gates) |
