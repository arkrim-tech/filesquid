# ImageSquid: Product Vision

<img src="./assets/ImageSquid-logo.svg" alt="ImageSquid logo" width="128" height="128">

**Believe your eyes**

**Status:** Draft, 2026-10-15. ImageSquid is a distribution of FileSquid: a photo-album UI layer over the FileSquid API. Platform requirements (storage, metadata, provenance, hosts, sync, performance, AI and publication modules) are in the FileSquid vision; this document holds what is specific to ImageSquid. Screenshots: [ui-mockups/](./ui-mockups/README.md). The screenshots are canonical for ImageSquid surfaces: where they and written requirements disagree, the screenshot governs.

## Table of Contents

- [Product Vision](#product-vision)
- [Consumer User Profile](#consumer-user-profile)
- [Product roadmap](#product-roadmap)
- [Technical Stack](#technical-stack)
- [Product requirements](#product-requirements)
- [Terminology](#terminology)
- [Standards, Laws, and Regulations](#standards-laws-and-regulations)

# Product Vision

ImageSquid exists to help people reclaim control of their visual memories.

Photographs and videos are among our most personal and irreplaceable possessions. Yet managing them increasingly feels like accepting that Apple, Google, Facebook or another platform will control how they are stored, organized, accessed, and shared.

We've grown accustomed to that dependence, even as we invest years in curating collections we cannot easily move. ImageSquid starts from the conviction that control over our memories should remain ours.

## What is ImageSquid?

ImageSquid is an open-source, local-first photo and video manager designed to help individuals, families, and groups reclaim their media and the curation they have already done across platforms such as Apple Photos, Google Photos, Microsoft, Instagram, Facebook, Twitter, Bluesky, etc.

It includes all the features you would expect from a mainstream photo album application, including AI-powered collections management and image editing tools.

### ImageSquid is a FileSquid distribution

ImageSquid is FileSquid with a photo-album user interface: library views by Years, Months, Days and All Photos, albums, galleries, exhibits, a viewer with an inspector (Metadata, C2PA and Archive tabs), and a non-destructive studio. Underneath, every photo and video is a FileSquid file in an OCFL object, with Dublin Core metadata and C2PA Content Credentials.

ImageSquid contains no backend code. Every capability it needs, including thumbnails and display derivatives, editing recipes, people and places, migration from photo platforms, AI connections and paid C2PA publication, is a FileSquid module reached through the FileSquid API. ImageSquid supplies views, design tokens, a label table (repo, library, album, gallery, exhibit), brand assets and its module set. It runs on every FileSquid host: the desktop app, a `filesquid-server`, and the encrypted browser host.

| Release                 | Product                                                                                  |
| ----------------------- | ---------------------------------------------------------------------------------------- |
| First                   | **FileSquid**: the platform and its default Drive-style UI, with OCFL, metadata and C2PA |
| Then                    | Encrypted backup and sync at `*.filesquid.tech`                                          |
| Second consumer release | **ImageSquid**: this distribution                                                        |
| Later                   | **Arkrim**: the digital-archives distribution                                            |

## User convenience

The ImageSquid user experience combines the everyday convenience of a mainstream photo app with cross-platform ownership, reliable migration, and institutional-grade backup.

For example, a single ImageSquid user library can be synced and used across Windows, macOS and Linux desktops, an online web app or mobile-web interfaces.

## Full media ownership

Media ownership means more than just access to image files: it means retaining available descriptions, dates, edits, organization, relationships, and source context that give those images meaning.

ImageSquid aims to make independent media ownership convenient. Users should have a fast, capable application for finding, enhancing, enjoying, and sharing their collections across devices and operating systems.

## Data portability

ImageSquid libraries remain usable offline, portable between providers, and exportable if users quit ImageSquid. Optional hosting and synchronization supports that freedom.

ImageSquid offers all application features as free and open-source software, with optional encrypted backup and syncing. ImageSquid offers that service under its own brand (imagesquid.app) as a white label of FileSquid's backup and sync features (FileSquid C-031).

This is a similar approach to the Obsidian note management tool but better because, Obsidian is free but closed sourced. With ImageSquid's open-source license users can clone the same software from our Git repo and host their own backup and sync infrastructure if they choose.

_To be clear_, ImageSquid users _MUST_ be able to leave a hosting provider, work offline, recover from mistakes, and leave ImageSquid the app and the optional online service at any time without losing original media, subsequent enhancements or metadata enrichment.

This principle and its practical implementation is based heavily on the Personal Data Server primitive used in the AT Protocol architecture by platforms like Bluesky.

## Long-term access

Another key point to make about ImageSquid is that it is designed from the perspective of professional archival collections management.

ImageSquid adopts a long-term perspective in regards to the stewardship, findability, usability, and trustworthiness of the digital files in the system's care.

Original media and its accumulated context are preserved using open archival standards so collections remain recoverable, understandable, and reusable over time.

A key component of this approach is the capability to use the files that ImageSquid stores on disk as the canonical data source. If any of the application layer or its indexes are corrupted or removed the user will have the option to rebuild the application layer in full using the source files stored in [OCFL](https://ocfl.io/1.1/spec/) (Oxford Common File Layout) objects.

OCFL is the digital archives industry's best practice for storing versioned files and inventories on disk so a collection remains inspectable and rebuildable with ordinary file tools, independent of ImageSquid's catalog or an online platform.

ImageSquid brings this discipline and professional design into everyday image management without requiring users to become archivists or system administrators.

## Image provenance

Provenance is part of this same trust narrative wherein archivists have been counted on for centuries to protect and demonstrate the authenticity of documents.

In practical terms, this means that every ImageSquid import starts a durable [C2PA](https://spec.c2pa.org/specifications/specifications/2.4/specs/C2PA_Specification.html) (Coalition for Content Provenance and Authenticity) record so prior credentials, generator marks and ImageSquid's later actions remain auditable using the industry-standard method.

C2PA is an open standard for Content Credentials: signed, file-bound statements about how an image or video was created or changed.

Files shared from ImageSquid will be able to clearly declare their origin, whether human operator, AI-generated, or a combination of both. Optional publication can provide a third-party signed, C2PA watermarked certificate.

ImageSquid also adopts the W3C [PROV-O](https://www.w3.org/TR/prov-o/) PROVenance Ontology which is a widely-used vocabulary for recording who did what to which resource, providing structure for human and machine-readable digital audit trails. These underpin ImageSquid components that can assist in detecting AI deepfakes while improving AI transparency around image creation and processing.

## Private AI processing

The actual AI inference models used in ImageSquid processing tasks and end-user features should remain the user's choice.

ImageSquid can connect to a local AI runner and open-source model (eg. llama.cpp + Gemma4) using the industry-standard OpenAI API format or to the owner's own supported frontier-provider account; the user pays that provider directly. This is bring-your-own (BYO) inference. The AI Connections surface MUST explain that arrangement, including that encrypted backup does not receive inference traffic and that a remote endpoint can read what the owner sends.

## High-performance UX

ImageSquid makes everyday browsing, scrolling, selection, adjacent-image viewing and slideshow interaction feel near-instant, even for large collections containing large files and high-resolution media.

Useful thumbnails and display-ready images appear promptly without waiting for the whole collection to finish processing. Imports, uploads, thumbnail generation, indexing and provenance scans coexist with responsive interaction.

This requirement applies from the first release forward; it is not a final optimization phase.

# Consumer User Profile

| Segment                                              | Jobs and pains                                                                                                                                                 | Desired gain                                                                                                                                                                 | Potential paid need                                             |
| ---------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------- |
| Privacy-conscious individuals                        | Organize a lifetime of media without mandatory accounts, cloud administration, or privacy concerns                                                             | A dependable, private, offline desktop home                                                                                                                                  | Encrypted backup and personal device connectivity               |
| Families and other private groups                    | Combine scattered memories while retaining individual privacy and understandable permissions                                                                   | Personal spaces plus shared collections and galleries for authorized family, friends and groups. Without having to run a server.                                             | Pooled capacity, sharing, recovery, assistance                  |
| Large media library owners                           | Move years of originals, edits, dates, albums, and paired media without unexplained loss; browse large collections and large files without processing stalls   | Near-instant everyday navigation, smooth image presentation, resumable work and inspectable migration/recovery reports                                                       | Scoped migration help, large restores                           |
| Enthusiast photographers                             | Preserve RAW, color, versions, and video while doing ordinary editing                                                                                          | Original fidelity and capable curation                                                                                                                                       | Higher storage/protection tiers and support                     |
| Creators and researchers                             | Save images/videos without losing retrieval evidence, attribution context or later processing history                                                          | A private collection with inspectable sources and portable provenance                                                                                                        | Managed preservation and selected C2PA publication/watermarking |
| Professional EU photographers and content publishers | Publish AI-generated or materially AI-manipulated photos/video that could be taken as real, without a workable visible-and-machine-readable disclosure package | A repeatable export that keeps originals private, retains generator marks, applies a first-exposure EU-icon (or equivalent accessible) label, and verifies the outgoing file | Paid C2PA/watermark publication profile with AI disclosure.     |

## GLAM market evolution

If the ImageSquid consumer product becomes widely used and reaches a critical mass of installs, ImageSquid MAY evolve to serve Gallery, Library, Archives, Museum (GLAM) institutions as one of the post-1.0 product milestones.

This would include professional digital asset management (DAM) workflows, additional digital-curation features, improved professional metadata support and integration with established collections management systems.

Most notably it would include [IIIF](https://iiif.io/) (International Image Interoperability Framework) import and publishing capabilities which are in popular use within GLAM institutions and researchers that use their collections.

IIIF is a set of open APIs used by galleries, libraries, archives and museums so images, pages and related media can be viewed, zoomed, compared and reused across different systems without locking them to one vendor's viewer.

## Enterprise market evolution

Professional records management and digital archiving for mixed record types (documents, email, datasets, audiovisual files and other formats) is FileSquid's scope, and its archives UI is the **Arkrim** distribution. ImageSquid stays the photo and video distribution. Because both run on the same FileSquid modules and OCFL store, an owner's ImageSquid library is already an archive that Arkrim or the FileSquid default UI can open.

# Product roadmap

ImageSquid is the **second consumer release**, after FileSquid's encrypted backup and sync and then the FileSquid release. There are no calendar dates. Interim ImageSquid releases lead to a complete consumer ImageSquid 1.0.

| <img src="./assets/ImageSquid-logo.svg" alt="ImageSquid logo" width="128" height="128"> | <img src="./assets/FileSquid-logo.svg" alt="FileSquid logo" width="128" height="128"> |
| :------------------------------------------------------------------------------------------------------: | :----------------------------------------------------------------------------------------------------: |
|                                              **ImageSquid**                                              |                                             **FileSquid**                                              |
|                            Consumer photo and video distribution of FileSquid                            |                    The platform: API, modules, hosts and the default Drive-style UI                    |

After ImageSquid 1.0, the owner chooses the order of the later milestones defined in the FileSquid vision: the Arkrim archives distribution, IIIF/GLAM interoperability (an ImageSquid and Arkrim capability built as FileSquid modules), and ProofMode integration (user-installed ProofMode capture; ImageSquid verifies and respects capture C2PA, stores packs in OCFL, and chains later enhance and export).

# Technical Stack

ImageSquid uses the FileSquid technical stack. The ImageSquid layer itself is:

| Part             | Choice                                                                                                         |
| ---------------- | -------------------------------------------------------------------------------------------------------------- |
| Location         | `apps/imagesquid/` in the FileSquid repository; extractable to its own repository                              |
| Views            | Vanilla TypeScript custom elements (no UI framework), built with Vite, composing `@filesquid/ui` components    |
| Theme            | Design tokens as CSS custom properties, light and dark                                                         |
| Vocabulary       | Label table mapping internal names to repo, library, album, gallery, exhibit                                   |
| Desktop branding | Tauri configuration overlay (product name, identifier, icons, UI bundle) applied to the FileSquid desktop host |
| Backend          | None; all calls go to the FileSquid API                                                                        |

# Product requirements

Requirement IDs are stable across the FileSquid and ImageSquid visions: `FE-`, `C-` and `T-` numbers missing here are FileSquid requirements. `FileSquid U-xx` refers to the FileSquid user requirements.

## Core Features

### Everyday organization

| ID     | Requirement                                                                                                                                                                                                                      | Related                            |
| ------ | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------- |
| FE-005 | Grids and viewers MUST be highly responsive, with bounded continuous scrolling, reusable views and nearby-media prefetch.                                                                                                        | U-01, FileSquid U-11; T-013, T-017 |
| FE-006 | The product MUST provide albums (internal collections), nested folders, favorites, galleries (internal virtual collections, including smart galleries as saved queries), keywords, metadata, geographic correction and utility galleries. | U-01, FileSquid U-04, U-05         |
| FE-007 | Interaction and useful-image readiness MUST meet the declared performance budgets during background work.                                                                                                                        | FileSquid U-11; T-010              |

### Non-destructive creativity

| ID     | Requirement                                                                                                                                                                                                                                                                                                                        | Related        |
| ------ | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------- |
| FE-008 | The product MUST provide reversible photo adjustments, RAW development, composition, filters, retouching, AI-assisted cleanup, annotation and linked versions.                                                                                                                                                                     | U-02           |
| FE-009 | Assistive standard editing that does not substantially alter the input or its semantics (crop, exposure, and equivalent) MUST NOT be classified as AI-generated content.                                                                                                                                                           | C-017          |
| FE-010 | When ImageSquid, under its own name, performs a retained material AI manipulation (generative fill, person removal, face swap, or equivalent), it MUST record the action in the C2PA/PROV-O history and MUST attach a machine-readable AI-generated-or-manipulated mark to the retained derivative as far as technically feasible. | FileSquid U-78 |

### Private discovery

| ID     | Requirement                                                                                                                                                                                             | Related     |
| ------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------- |
| FE-012 | The product MUST provide people and pets, groups, places and Trips, OCR, natural-language and video-moment search, visual identification and subject extraction, with correction and analysis controls. | U-01        |
| FE-013 | Household people/pets organisation MUST remain available without a deployer-style notice.                                                                                                               | U-04        |
| FE-014 | When people or biometric categorisation analysis is used under professional authority, ImageSquid MUST support informing natural persons exposed to that operation.                                     | U-04        |
| FE-015 | Emotion recognition, untargeted facial-image scraping and remote biometric identification in publicly accessible spaces MUST NOT be implemented.                                                        | U-04; C-020 |
| FE-016 | GDPR and biometric-analysis review required in the standards table remain a launch gate. Notice MUST NOT be claimed as GDPR compliance.                                                                 | U-04        |

### Cross-platform access

| ID     | Requirement                                                                            | Related        |
| ------ | -------------------------------------------------------------------------------------- | -------------- |
| FE-018 | Native Apple acquisition MUST remain a macOS adapter rather than a Linux prerequisite. | FileSquid U-14 |

### Enjoyment and presentation

| ID     | Requirement                                                                                                                                                                                                                                                                                                                                                                       | Related               |
| ------ | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------- |
| FE-023 | The product MUST provide Memories, prompt-assisted storytelling, editable slideshows, soundtracks, printing/PDF and authorized photo-product partners.                                                                                                                                                                                                                            | U-03                  |
| FE-024 | Slideshows MUST prepare upcoming display-sized media in advance within resource limits, retain responsive playback controls and show explicit unavailable or processing states rather than silently skipping items or freezing on a large original.                                                                                                                               | FileSquid U-11; T-017 |
| FE-025 | AI systems in ImageSquid that interact directly with a person (including a collection assistant and prompt-assisted memories driven by a model) MUST inform that person they are interacting with AI at the start of the first interaction, in a clear, distinguishable and accessible way, unless it is obvious to a reasonably well-informed, observant person in that context. | FileSquid U-82        |
| FE-026 | Memories MUST NOT add emotion recognition.                                                                                                                                                                                                                                                                                                                                        | U-04; C-020           |

### Advanced media and integration

| ID     | Requirement                                                                                                                                                                              | Related        |
| ------ | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------- |
| FE-027 | The product MUST support Live/motion photos, Portrait/depth, Cinematic, HDR, spatial media, video/audio tools, external editors, system sharing and extension contracts where supported. | FileSquid U-14 |

### Migration with evidence

| ID     | Requirement                                                                                                                                                                                | Related        |
| ------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | -------------- |
| FE-028 | The product MUST support Apple exports and a macOS PhotoKit route, Google Takeout, Windows folders and hydrated OneDrive content, with optional selected-item and direct-cloud connectors. | FileSquid U-08 |

## User Requirements

| ID   | User statement                                                                                                             | Product consequence                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                 |
| ---- | -------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| U-01 | I want to find and organize memories without managing a database.                                                          | Accessible photo browsing, metadata, albums, semantic discovery and repair workflows, presented by the ImageSquid distribution over FileSquid modules.                                                                                                                                                                                                                                                                                                                                                                                                                              |
| U-02 | I want to improve a photograph without losing its original.                                                                | Versioned non-destructive photo editing (studio) and faithful export, presented by the ImageSquid distribution; the editing engine is a FileSquid module.                                                                                                                                                                                                                                                                                                                                                                                                                           |
| U-03 | I want to enjoy, print and share my collection.                                                                            | Memories, slideshows, annotations, print output and authorized integrations.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                        |
| U-04 | If I use people recognition for professional work, people in the photos should be able to be informed of that operation.   | Household people/pets organisation without a deployer-style notice; professional biometric-categorisation use MUST support informing exposed persons; no emotion recognition or untargeted face scraping.                                                                                                                                                                                                                                                                                                                                                                           |
| U-05 | I want the ImageSquid interface to talk like a photo app: repo, library, album, gallery and exhibit.                       | ImageSquid UIs, help and ImageSquid UI requirements MUST use repo, library, album, gallery and exhibit as specified in [Terminology](#terminology). They MUST NOT show repository, collection, virtual collection or virtual exhibit as ImageSquid UI nouns. Primary chrome MUST label the preservation store **Archives** (for example Archives Synced). **OCFL** MAY appear in the advanced inspector, diagnostics and CLI. Collection skim origin labels MUST be the two-letter capitals **AI** or **RW** (**RW** is Real World). The Content Credentials pin MAY remain **CR**. **Screenshot exceptions (the [ImageSquid screenshots](./ui-mockups/README.md) are canonical):** the sidebar heading over the automatic galleries reads **Collections**; the search placeholder reads “Search photos, collections, dates…”; the C2PA floatover's origin badge uses the long forms **Real** or **AI** while tile chips stay **AI** / **RW**; the smart query **Hardware Certified** selects files whose C2PA capture claim is hardware-signed and is not a certification claim (FileSquid C-018). |
| U-06 | I want ImageSquid to be FileSquid underneath, so my photos stay in the same open archive and work with any FileSquid host. | ImageSquid MUST be a FileSquid distribution: views, theme tokens, a label table, brand assets and a module set. It MUST NOT contain backend code. Every capability it needs MUST be a FileSquid module reached through the FileSquid API.                                                                                                                                                                                                                                                                                                                                           |

## Key Constraints

### Platforms, originals and service limits

| ID    | Requirement                                                                                                                                                                                                     | Related |
| ----- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------- |
| C-005 | ImageSquid MUST NOT claim transfer of Apple adjustment history, source sharing permissions, faces or dynamic memories unless a route is implemented and tested. Automatic source deletion MUST NOT be included. | FE-028  |

### Parity, preservation and sequence

| ID    | Requirement                                                                                                                                                                                                       | Related        |
| ----- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------- |
| C-036 | The mature product MUST address every feature family that mainstream photo managers offer through an equivalent user outcome, a documented platform adapter, or an explicit capture/compatibility limitation.      | FE-027, FE-028 |
| C-037 | Equivalence MUST NOT promise identical Apple rendering, access to proprietary services, or reconstruction of information a source never exports.                                                                  | C-005          |
| C-039 | Migration completion and feature parity MUST remain separate claims.                                                                                                                                              | FE-028, FE-029 |
| C-040 | Within ImageSquid, ownership and dependable organization MUST precede deeper editing, discovery, connectivity, storytelling, advanced media, and a final parity audit. These are gates, not calendar commitments. | C-024, C-025   |

## Terminology

ImageSquid uses FileSquid's internal names in code, data and APIs, and these labels in its UI:

| Internal name (FileSquid terminology) | ImageSquid UI label                                                                        |
| ----------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------ |
| **repository**                                                                            | **repo**                                                                                   |
| **library**                                                                               | **library**                                                                                |
| **collection**                                                                            | **album**                                                                                  |
| **file**                                                                                  | **photo**, **image** or **video** (media kind); **file** remains the item in data and APIs |
| **virtual collection**                                                                    | **gallery**                                                                                |
| **virtual exhibit**                                                                       | **exhibit**                                                                                |

ImageSquid end-user interfaces, help and `srs.md` UI requirements MUST use the ImageSquid labels (**repo**, **library**, **album**, **gallery**, **exhibit**). They MUST NOT present **repository**, **collection**, **virtual collection** or **virtual exhibit** as ImageSquid UI nouns where this table defines a distinct ImageSquid label. **Library** is the same word in both layers. ImageSquid UIs MAY call a given file a photo, image or video according to its media kind. The item-level noun remains **file** at all levels, including ImageSquid backends. Primary chrome MUST use **Archives** for the preservation store. **OCFL** MAY appear in advanced inspector, diagnostics, CLI, and FileSquid. When a collection skim shows origin labels, those labels MUST be **AI** or **RW** (capitals; **RW** is Real World). The Content Credentials pin MAY remain **CR**. **Screenshot exceptions (the [ImageSquid screenshots](./ui-mockups/README.md) are canonical):** the sidebar heading over the automatic galleries reads **Collections**; the search placeholder reads “Search photos, collections, dates…”; the C2PA floatover's origin badge uses the long forms **Real** or **AI** while tile chips stay **AI** / **RW**; the smart query **Hardware Certified** selects files whose C2PA capture claim is hardware-signed and is not a certification claim (FileSquid C-018).

The FileSquid default distribution MUST use the internal names and MUST NOT inherit these ImageSquid labels as its product nouns.

ImageSquid presents physical collections as albums and virtual collections as galleries. Multiple album membership without duplicating originals is a gallery (or other non-storage membership) outcome, not a second collection storage location. A gallery is either a smart gallery (saved query) or a hand-picked gallery (explicit, manually ordered list); a photo dragged into a second album joins a hand-picked gallery. Public cross-library and cross-repository curation is an exhibit.

The brand promise is **“Your photos. Private, preserved, traceable, portable.”**

## Standards, Laws, and Regulations

ImageSquid is bound by the FileSquid standards, laws and regulations. This row applies with particular force to the photo and video distribution:

| Area                 | Intended application and decision boundary                                                                                                                                                                                                                                                                                                                                                                            |
| -------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Photo/video metadata | EXIF, IPTC and XMP where supported; native video container/track/timed metadata with IPTC Video Metadata Hub mappings. Read, sidecar correction, native writing and privacy stripping require separate capability evidence; video has no single universal EXIF equivalent. [XMP](https://developer.adobe.com/xmp/docs/xmp-specifications/), [IPTC Video Metadata Hub](https://iptc.org/standards/video-metadata-hub/) |
