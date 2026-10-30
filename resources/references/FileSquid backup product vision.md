# FileSquid encrypted backup

Filled on 29 October 2026 from [`resources/templates/product-vision-template.md`](../templates/product-vision-template.md). This is the canvas for the release that follows the FileSquid MVP. It is the same product as FileSquid. The Drive stays the daily interface. OCFL stays behind that Drive.

Canonical requirements stay in the FileSquid vision and the PRD: [C-025](../product-vision.md#release-sequence), [C-031](../product-vision.md#platforms-originals-and-service-limits), [F-029](../prd.md#f-029-client-encryption-and-key-custody), [F-030](../prd.md#f-030-personal-synchronization-and-availability), [F-031](../prd.md#f-031-encrypted-backup-and-verified-restore), [F-032](../prd.md#f-032-accounts-entitlements-billing-and-cancellation), [F-043](../prd.md#f-043-c2pa-records-and-action-disclosure), [F-046](../prd.md#f-046-media-metadata-inspection-correction-and-privacy-exports) and [F-059](../prd.md#f-059-hosted-encrypted-backup-and-sync-service). The local `U-` numbers below are reading aids for this note. They are not allocated into the PRD.

The order is the owner decision of 29 October 2026 ([ADR 029](../architecture/029-filesquid-unified-product-and-distributions.md)): the macOS MVP (Task FS-3.9), then this backup release (phase F4, Task FS-4.2) with the Ubuntu (GNOME) and Windows Tauri desktop releases (Task FS-3.11) beside it, then the rest of the first consumer release (phase F3R, Task FS-3.5), then ImageSquid. The design record is the [backup design report](FileSquid%20backup%20design%20report.html).

## Product Vision

After the macOS MVP, FileSquid's primary product is encrypted backup and sync at `*.filesquid.tech`. People keep doing their daily file work in the Drive: folders, files, search, and versions they can see and restore. Behind that Drive, a recovery point is a pinned encrypted OCFL generation. The owner restores a point in time. The owner does not operate a repository format.

Backup and sync are different guarantees. The working library is layer 1. A retained recovery point is layer 2. A second copy of those points is layer 3. The live sync head is not a recovery point. The decisive test is: delete a file, let sync run, restore yesterday's recovery point, and the file is back.

The free Drive on the owner's computer keeps working offline. The paid service is the ciphertext-only mailbox. A library can still leave the service, because the storage format behind the Drive is OCFL.

## Target Users & Customer Profile

The same person the FileSquid Drive is for: someone who wants a private, durable place for their files, from a personal library up to an institutional quota on the same account model ([C-031](../product-vision.md#platforms-originals-and-service-limits)).

The job after the MVP is protection that survives a deleted file, a bad sync, or a dead disk, without handing the operator the keys or asking the owner to run a second archive product. The gain is a restorable point, visible in the Drive, with the local library still theirs if they cancel.

Several devices may upload into one Drive. The owner enrolls each device's public key onto that node's whitelist. A key that is not listed is refused. The upload crosses a network the owner does not trust, so the bytes are encrypted before they leave the uploading device.

Ubuntu (GNOME) and Windows Tauri desktop releases start when the MVP closes and run beside this release. They are not a gate on the first backup, and this release is not a gate on them. `filesquid-server` and ImageSquid follow this release. The first backup is already a Drive whose recovery points sit in more than one geography.

## Core Features

- Client encryption and key custody before any upload: recovery phrase shown once, device wrap, revocation ([F-029](../prd.md#f-029-client-encryption-and-key-custody)).
- The Drive owner grants upload permission by enrolling each device's public key onto that node's whitelist. Several devices may be on it. A password is not that permission. Revoking a key stops later uploads, and the key cannot delete a recovery point ([F-029](../prd.md#f-029-client-encryption-and-key-custody)).
- Recovery points that stay independent of live sync deletions, with restore of a chosen point onto a clean device and a plaintext check on the client. The owner commits and pushes the current snapshot on demand, or on a schedule of 1 hour or 24 hours. Each Drive's backup keeps those points in more than one geography ([F-031](../prd.md#f-031-encrypted-backup-and-verified-restore)).
- Personal sync as a separate guarantee, turned on only after a clean-device restore has passed. Whitelisted devices on one node sync the live working set with Automerge. Automerge is not a recovery point ([F-030](../prd.md#f-030-personal-synchronization-and-availability)).
- On upload, EXIF and any other embedded metadata, and any sidecar, move into that file's `<file-uuid>.metadata.md`. The stored bytes are the stripped content. The upload adds a Prov-O record and a C2PA proof on the ProofMode model, signed by the uploading device key ([F-046](../prd.md#f-046-media-metadata-inspection-correction-and-privacy-exports)).
- A ciphertext-only hosted service at `*.filesquid.tech`, with accounts, entitlements, visible quotas and cancellation that leaves the local library in place ([F-032](../prd.md#f-032-accounts-entitlements-billing-and-cancellation), [F-059](../prd.md#f-059-hosted-encrypted-backup-and-sync-service)).
- Resumable upload: the client encrypts, persists a checkpoint, and writes S3 multipart parts. Completion is idempotent. The offset survives a reboot of every FileSquid process ([ADR 012](../architecture/012-s3-compatible-ciphertext-storage.md)).
- Objects first, manifest last. Two hosted credentials: one may append, one may retain. The sync credential cannot delete recovery points.

## User Requirements

- **U-01.** I want encrypted backup to be the product I get as soon as the MVP is finished, in the same Drive I already use. Canonical: [C-025](../product-vision.md#release-sequence).
- **U-02.** I want daily file work to stay in that Drive, with OCFL, the encrypted OCFL profile and PROV-O behind it. Canonical: [C-025](../product-vision.md#release-sequence).
- **U-03.** I want a recovery point that brings a file back after I delete it and sync. Canonical: [F-031](../prd.md#f-031-encrypted-backup-and-verified-restore).
- **U-04.** I want sync and backup stated as different guarantees, in the product and in the task list. Canonical: [F-030](../prd.md#f-030-personal-synchronization-and-availability), [F-031](../prd.md#f-031-encrypted-backup-and-verified-restore), [C-031](../product-vision.md#platforms-originals-and-service-limits).
- **U-05.** I want the hosted service to store ciphertext only, and to hold no library keys. Canonical: [F-029](../prd.md#f-029-client-encryption-and-key-custody), [F-059](../prd.md#f-059-hosted-encrypted-backup-and-sync-service).
- **U-06.** I want an interrupted upload to continue from the server's part offset after every FileSquid process has restarted. Canonical: [ADR 012](../architecture/012-s3-compatible-ciphertext-storage.md).
- **U-07.** I want the free Drive to keep working on my computer when I have no subscription. Canonical: [C-031](../product-vision.md#platforms-originals-and-service-limits).
- **U-08.** I want a proved restore before anyone claims the paid sync is reliable. Canonical: [F-031](../prd.md#f-031-encrypted-backup-and-verified-restore).
- **U-09.** I want to leave with a library I can still read. Canonical: [F-042](../prd.md#f-042-ocfl-preservation-and-portable-recovery).
- **U-10.** I want the charge, the quota and the cancellation window stated before I pay, and my local library kept after I cancel. Canonical: [F-032](../prd.md#f-032-accounts-entitlements-billing-and-cancellation).
- **U-11.** I want to let several devices upload into my Drive by putting each device key on the node whitelist, and I want a password to be unable to grant that upload. Canonical: [F-029](../prd.md#f-029-client-encryption-and-key-custody).
- **U-12.** I want my Drive's backup to keep copies in more than one geography, so one place failing leaves another copy I can restore. Canonical: [F-031](../prd.md#f-031-encrypted-backup-and-verified-restore), [F-059](../prd.md#f-059-hosted-encrypted-backup-and-sync-service).
- **U-13.** I want the devices on my node's whitelist to sync the live working set with Automerge, and I want that sync to stay distinct from a recovery point. Canonical: [F-030](../prd.md#f-030-personal-synchronization-and-availability).
- **U-14.** I want to commit the current state to OCFL on demand, or every 1 hour or every 24 hours, as the snapshot backup package. Canonical: [F-031](../prd.md#f-031-encrypted-backup-and-verified-restore).
- **U-15.** I want an upload to move embedded and sidecar metadata into the file's Markdown record and to add a Prov-O record and a C2PA proof of that upload. Canonical: [F-046](../prd.md#f-046-media-metadata-inspection-correction-and-privacy-exports), [F-043](../prd.md#f-043-c2pa-records-and-action-disclosure).

## Technical Stack

The stack stays the one FileSquid already decided.

- Rust on the macOS desktop, with the existing `filesquid-ocfl` library as the client of the encrypted OCFL profile ([ADR 010](../architecture/010-ocfl-1-1-dual-profile.md), [ADR 016](../architecture/016-rust-ocfl-library.md)).
- Cryptography is [ADR 009](../architecture/009-cryptography-webcrypto-compatible-suite.md): AES-256-GCM, HKDF-SHA-256, SHA-256, Ed25519, X25519 and Argon2id, shared by Rust and WebCrypto. Fixity stays SHA-256 because that is the OCFL digest.
- The hosted mailbox is Axum. It authorises, checks length and checksum, and fences head publication. It does not parse OCFL and it does not hold keys ([ADR 004](../architecture/004-axum-hosted-ciphertext-mailbox.md)). The hosted-service binary is separate from `filesquid-server` ([ADR 032](../architecture/032-hosting-and-privacy-model.md)).
- Bytes go from the client to S3-compatible multipart storage. A tus process may sit in front of those same parts if a server must terminate the bytes. That process is not a second file identity.
- Resume is a durable offset plus idempotent completion. It is not a reason to move the desktop, the mailbox or `filesquid-ocfl` onto Elixir or BEAM ([ADR 012](../architecture/012-s3-compatible-ciphertext-storage.md), [ADR 013](../architecture/013-cargo-svelte-workspaces.md)).
- The operational database holds accounts and devices and stays empty of private titles.
- Automerge syncs the live working set among the devices on one node's whitelist. The hosted mailbox does not read that document. OCFL versions remain the snapshots.

Left out of this release: restic, Borg or Kopia as the store; age, ChaCha20-Poly1305 or BLAKE3 as the content suite; a phone PWA; Iroh. Automerge is in this release as live sync, and it is not the store.

## Standards, Laws, and Regulations

- OCFL 1.1, as the storage format and the exit path behind the Drive.
- PROV-O, as generated history behind the Drive. It is not an authoring form on the backup screen.
- GDPR role analysis is a launch gate before other people's ciphertext is stored ([F-059](../prd.md#f-059-hosted-encrypted-backup-and-sync-service)). This canvas does not decide controller or processor.
- The crypto-protocol audit, named providers and regions in more than one geography, and the payment processor are the other launch gates in F-059. Paying users wait on all four. No service level is invented here. No count of regions is fixed here.

## Technical & Domain References

- [Backup design report](FileSquid%20backup%20design%20report.html), the owner decision and the development slices.
- [Field media pipeline](FileSquid%20field%20media%20pipeline.html), the threat model, the resumable-upload rules, and the split between backup and sync.
- [ADR 009](../architecture/009-cryptography-webcrypto-compatible-suite.md), [ADR 010](../architecture/010-ocfl-1-1-dual-profile.md), [ADR 012](../architecture/012-s3-compatible-ciphertext-storage.md), [ADR 029](../architecture/029-filesquid-unified-product-and-distributions.md), [ADR 032](../architecture/032-hosting-and-privacy-model.md).
- Phase F4 of the [implementation plan](../implementation-plan.md) and the Backup section of the [checklist](../implementation-plan-checklist.md).

## Value Propositions

The owner gets a Drive they can use every day, and a recovery point that is not the same thing as “my other computer has synced.” A mistake that syncs does not erase yesterday. The operator of `*.filesquid.tech` cannot read the library. Cancelling the subscription leaves the local Drive and an OCFL library the owner can still open.

## Unfair Advantage / The Moat

The daily interface is a Drive. The bytes underneath are an encrypted OCFL library the owner can take elsewhere. Mainstream sync products collapse backup into the live head. Repository tools such as restic, Borg and Kopia make the owner operate the archive. FileSquid keeps both the Drive and the exit path, with keys on the client and a mailbox that stores ciphertext only.

## Key Constraints

- This release starts when Task FS-3.9 is checked. Task FS-3.11 (Ubuntu and Windows desktop) starts then too and does not wait on Task FS-4.2. The rest of phase F3R waits until Task FS-4.2 is checked. The desktop backup path does not wait for `filesquid-server`.
- Task FS-4.1 depends on Task FS-3.9 and Task 0.2.
- Upload permission for a Drive is a device public key on that node's whitelist. Several devices may be on it. A password is not that permission.
- Live sync among those devices is Automerge. A recovery point is an OCFL snapshot, on demand or every 1 or 24 hours.
- An upload strips embedded and sidecar metadata into `<file-uuid>.metadata.md` and adds a Prov-O record and a C2PA proof of that upload.
- Each Drive's backup keeps recovery points in more than one geography. Further destinations, physical seeding and paid assurance exercises stay F-035.
- Restore is demonstrated before a paid sync reliability claim.
- The content suite stays ADR 009. The store stays the encrypted OCFL profile.
- Prices, quotas and the cancellation-window length are policy decisions required before launch. This note fixes none of them.

## Market Analysis

The governing documents name the buyer and the job. They do not estimate a market size, and this canvas does not add one. Evidence that someone will pay is a gate, not a figure: hosted quota, retention and restore-traffic terms have to be visible before purchase, and no reliability claim is made before a representative restore has passed ([F-031](../prd.md#f-031-encrypted-backup-and-verified-restore)).

## Key Competitors

Mainstream cloud drives sell a live replica and call it safety. A delete that syncs is gone. restic, Borg and Kopia sell a real repository, and they ask the owner to run it. FileSquid borrows their repository rules (content-addressed objects, append-only writes, manifest last, prune as an explicit policy) and keeps them behind the Drive, in the encrypted OCFL profile. It does not ship those tools as the product.

## Revenue Streams

The decided earning model is a user subscription for managed encrypted OCFL backup and/or encrypted account sync at `*.filesquid.tech` ([C-031](../product-vision.md#platforms-originals-and-service-limits)). Backup and sync stay distinct line items of that one account. The same account scales from a person to an institution. ImageSquid may later sell the same subscription under its own brand, on the same infrastructure.

The Drive, local capture, metadata controls, preservation packaging and export stay free. This canvas uses the product-vision template. It does not set a separate legal form, and it does not set a price. A later paid layer of OCFL batch jobs ([C-051](../product-vision.md#platforms-originals-and-service-limits)) does not replace this subscription.

## Sales Channels

The macOS desktop is the first client, as an early-access build of the Drive, then as a subscriber of `*.filesquid.tech`. Ubuntu (GNOME) and Windows Tauri desktops join as Release 1 beside this release. The account is the channel for the paid service. `filesquid-server` enrols in the same service after this release. No separate field-capture app ships with the first backup.

## Key Resources & Partnerships

- The FileSquid desktop, `filesquid-ocfl`, `filesquid-api` and the hosted-service binary created for this release.
- S3-compatible providers and regions, still the open choice in ADR 012, more than one geography for each Drive, plus object lock or an equivalent retention floor.
- An independent crypto-protocol review before other people's ciphertext is stored.
- A payment processor, and the GDPR role analysis, before the paid service opens.

## Cost Structure

The documents name the costs and do not price them. Variable cost is hosted object storage, transfer and restore traffic. Fixed cost before other people's ciphertext includes the crypto-protocol audit, the payment processor and the legal role analysis. The free Drive carries no hosted capacity. Free tooling does not imply free unlimited storage or transfer.

## Key Metrics

- The decisive restore: delete, sync, restore yesterday's point, the file is back, with the plaintext digest checked on the client.
- RC-003 and RC-006 on the macOS desktop against the hosted service: backup and sync stay distinct, and a library restores without the live service catalog ([Task FS-4.2](../implementation-plan.md#task-fs-4.2)).
- A ciphertext-only dependency graph: the hosted binary links no private-plaintext module, no libvips and no FFmpeg.
- A killed upload resumes from the store's offset, and a second run of a finished upload is a no-op.
- An enrolled uploader key can append ciphertext and cannot delete a recovery point. A password cannot grant that upload.
- One Drive's recovery points are present in more than one geography, and a copy remains after one geography is unavailable.
- No reliability claim, and no service level, until those exercises have passed.

---
© Pieter Van Garderen (2026)
