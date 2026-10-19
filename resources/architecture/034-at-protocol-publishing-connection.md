# Architecture Decision Record - FileSquid

034-at-protocol-publishing-connection.md

## Title

Opt-in AT Protocol (Bluesky) publishing connection instead of an RSS activity feed

## Status

Accepted

Accepted: 2026-10-16. FileSquid does not offer an RSS or bearer-link feed of the activity stream; instead a user MAY connect their own AT Protocol account and publish chosen items to it. This ADR records the integration boundary. It does not select the final crate or the authentication flow; those are implementation choices gated below.

## Context / Requirement Reference

- Requirements: [PRD F-064](../prd.md#f-064-file-activity-comments-and-user-activity-stream) (publishing bullet), [F-046](../prd.md#f-046-media-metadata-inspection-correction-and-privacy-exports) (outgoing disclosure policy), [F-053](../prd.md#f-053-virtual-exhibit) (public-only members), [F-043](../prd.md#f-043-c2pa-records-and-action-disclosure) (publication is a disclosure event), [F-039](../prd.md#f-039-consent-notifications-and-activity-privacy), [FileSquid U-06](../product-vision.md#platform-ownership-and-access), [FileSquid U-20](../product-vision.md#one-api-hosts-and-distributions).
- Nextcloud exposes the activity stream as an RSS feed whose link carries a secret token granting unauthenticated access. For a privacy-first product that is a standing disclosure channel. The owner prefers a deliberate, per-post publication to a social network the user already uses.
- AT Protocol is an open, documented protocol with a public lexicon (`app.bsky.feed.post`, `com.atproto.repo.createRecord`, blob upload) and a personal data server the user chooses. Rust client crates exist on crates.io and were verified on 2026-10-16 with `cargo search`: `atrium-api` 0.25.8 (API library for AT Protocol) and `bsky-sdk` 0.1.24 (ATrium-based SDK for Bluesky).

## Decision

1. **No activity feed.** FileSquid MUST NOT expose the activity stream as RSS, Atom or any bearer-link feed. Activity is read inside the app through the authenticated API ([F-064](../prd.md#f-064-file-activity-comments-and-user-activity-stream)).
2. **Publishing connection as a module behind the API.** A `publish-atproto` capability in FileSquid's application layer (`filesquid-api`, enabled per distribution through the module set, [F-058](../prd.md#f-058-white-label-distributions)) lets a user connect one or more AT Protocol accounts (handle plus personal data server) and publish a chosen file or a chosen activity entry as a post. Every publication is one explicit user request; nothing is published automatically, on a schedule or by rule.
3. **What may be published.** Only a file whose home collection or virtual exhibit marks it public ([F-053](../prd.md#f-053-virtual-exhibit)), or an activity entry the user may already read and chooses to share. Hidden files, private collections, journal rows and other users' activity are not publishable. Published media MUST pass the [F-046](../prd.md#f-046-media-metadata-inspection-correction-and-privacy-exports) outgoing policy (scrubbing, retained fields, FileId filename) and the outgoing-disclosure preview MUST be shown before confirmation.
4. **Where it runs.** The connection and the publish operation run only on an authorised endpoint (the desktop host, an owner-operated `filesquid-server`, or the encrypted browser host). The ciphertext-only `*.filesquid.tech` service MUST NOT link this capability or hold account credentials ([ADR 032](./032-hosting-and-privacy-model.md)).
5. **Credentials.** Account credentials (an app password or OAuth tokens, whichever the implementation adopts) are stored through the host's secret store (OS keychain on desktop, the server's secret store on `filesquid-server`), never in the journal, stdout, the OCFL root or a label table. Disconnecting revokes stored credentials.
6. **Journal and provenance.** Connect, publish, publish failure and disconnect are journaled ([F-061](../prd.md#f-061-activity-journal-and-live-health-monitoring)) and appear in the user's activity stream. A successful publication records a disclosure action on the file's C2PA record ([F-043](../prd.md#f-043-c2pa-records-and-action-disclosure)) with the post URI as an opaque reference.
7. **Honest limits.** The remote network re-encodes images and strips embedded metadata; FileSquid MUST NOT claim that C2PA manifests, watermarks or retained EXIF survive on the remote side. The post is a copy; revoking it later is the remote account's deletion, outside FileSquid's control, and the UI says so.

## Rationale

- Removes a bearer-token disclosure channel while giving the owner the outbound sharing they asked for.
- Reuses existing boundaries (public-only members, outgoing disclosure policy, journal, module set, host privacy model) instead of inventing a publication path.
- AT Protocol is open and self-hostable, consistent with the vision's ownership and vendor-independence principles; the user's account and data server are theirs.

## Assumptions

- The user already has an AT Protocol account; FileSquid does not create accounts or run a personal data server.
- Image and short-video posts cover the owner's intent; long video, threads and rich text beyond a caption and alt text are out of scope until requested.
- Alt text for an image post is taken from `dcterms:description` or the accessibility description when the user does not override it ([F-005](../prd.md#f-005-favorites-metadata-and-keywords), [NFR-006](../prd.md#nfr-006-accessibility-and-usability)).

## Alternatives Considered

- **RSS or Atom feed with a secret token (Nextcloud model).** Rejected: unauthenticated standing access to activity text, contrary to [NFR-004](../prd.md#nfr-004-security-and-privacy) and [F-039](../prd.md#f-039-consent-notifications-and-activity-privacy).
- **Authenticated feed (feed reader signs in).** Rejected for now: no mainstream reader supports it, and the owner did not ask for a feed.
- **ActivityPub (Mastodon) connection.** Not rejected; deferred until requested. The publish boundary above is network-agnostic, so a second network would be another adapter behind the same capability.
- **Automatic posting of new public items.** Rejected: publication must be a deliberate act under [F-039](../prd.md#f-039-consent-notifications-and-activity-privacy).

## Consequences / Implications

- Easier: outbound sharing to a network the user chooses, with the same disclosure preview as every other FileSquid sharing route.
- Harder: one more external dependency to pin and audit (crate choice recorded at implementation in the module README, as [ADR 011](./011-native-media-adapters-and-ffmpeg-license.md) and [ADR 026](./026-filesquid-provenance-engine.md) do), credential storage per host, and remote API changes to track.
- Tests: publication MUST be covered by a scenario in the F-064 feature file (public-only, policy applied, journaled, no automatic posting) and by the [F-040](../prd.md#f-040-compatibility-and-parity-register) register row that states what does not survive on the remote side.

## Related Decisions / Notes

- [ADR 030](./030-filesquid-api-and-transports.md) (one API), [ADR 032](./032-hosting-and-privacy-model.md) (host privacy model), [ADR 033](./033-activity-journal-and-live-health-monitoring.md) (journal).
- Implementation gates: crate selection (`atrium-api` or `bsky-sdk`, both verified to exist) and the authentication flow (app password versus OAuth) are decided in the API module plan task that implements F-064 publishing (API-F2.4); the crate and version are then pinned in the lockfile and the module README.
- A vision row for outbound publishing is a candidate follow-up so the PRD allocation stays one-to-one.
