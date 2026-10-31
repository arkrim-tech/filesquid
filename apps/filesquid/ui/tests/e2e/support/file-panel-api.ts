/**
 * Fakes of the routes the file panel reads and writes (UI-F2.3, UI-F2.11) for the production-build
 * Playwright tests: `files/details`, `files/history`, `files/versions`, `files/history-size`,
 * `files/download` (API-F1.3, API-F2.5), `files/restorations` and `files/labels`. They answer the mockups' data: mockup 35's
 * `facade-study.jpg` (four versions) and mockups 26 and 27's `harbour-masterplan-v7.pdf` (seven
 * versions). The restore is a command (API-F2.13): its body is `{ envelope, input }`, a
 * committed answer carries the outcome with its `result`, and an expected revision that is not
 * the head is a 409 `conflict` outcome; a repeated operation ID of the same command answers its
 * first committed answer again, before the revision is checked, as the API's receipts do. The label
 * edit is the same kind of command over `PUT`: a label that changes nothing is a 409, a committed
 * one publishes a version that holds the labels record (it appears among the object's versions
 * but not in the file's history) and sets the label of the version it names; a repeated
 * operation ID answers its first outcome again, and the same ID with another route or input is a
 * 400, as the receipts of both commands do. The download route answers a small body as an
 * attachment named by the display name; a ranged `files/content` read of a version above the
 * desktop's 16 MiB answer cap is answered with a 206, its `Content-Range` and a body of that
 * length, and recorded. The listing, the preview and the shell's routes come
 * from `serveFiles` and `serveNavigation`; these routes are registered after them, so they take
 * precedence.
 */
import type { Page, Route } from "@playwright/test";
import { DESKTOP_ANSWER_CAP } from "../../../src/files/download-source";
import { MemoryFileSource } from "../../../src/files/memory-source";
import { PNG, serveFiles } from "./files-api";
import { serveProvenance, type ProvenanceApi } from "./provenance-api";

/** The object both fixture files live in, a file object's `urn:uuid` (mockup 35). */
export const PANEL_OBJECT = "urn:uuid:7f3c9a1e-0b2c-4d5e-8f90-a1b2c341d2b8";
/** The library. */
export const PANEL_LIBRARY = "lib";
/** The route of the object's root folder. */
export const PANEL_ROUTE = `/#/files/${PANEL_LIBRARY}/${encodeURIComponent(PANEL_OBJECT)}/original`;
/** Stored names, which never reach the page. */
export const PANEL_STORED = ["9f2c41aa", "9f2c41ab"] as const;

/** One fixture file: its stored path, display name and versions (label, time, size). */
export interface PanelFixture {
  readonly path: string;
  readonly displayName: string;
  readonly mediaType: string;
  readonly versions: readonly (readonly [string | null, string, number])[];
}

/**
 * Mockup 35: facade-study.jpg, four versions, the head v4 at 11:05 UTC on 16 Oct 2026. Its
 * stored original is named by its file id, as API-S-026 stores it, so the metadata routes
 * (UI-F2.4) take it as the file's own original.
 */
export const FACADE: PanelFixture = {
  path: `original/${PANEL_OBJECT.slice("urn:uuid:".length)}.jpg`,
  displayName: "facade-study.jpg",
  mediaType: "image/jpeg",
  versions: [
    [null, "2026-10-12T14:20:00Z", 6_481_920],
    [null, "2026-10-13T09:00:00Z", 6_481_920],
    [null, "2026-10-15T09:00:00Z", 6_481_920],
    [null, "2026-10-16T11:05:00Z", 6_481_920],
  ],
};

/** Mockups 26 and 27: harbour-masterplan-v7.pdf, seven versions. */
export const HARBOUR: PanelFixture = {
  path: `original/${PANEL_STORED[1]}.pdf`,
  displayName: "harbour-masterplan-v7.pdf",
  mediaType: "application/pdf",
  versions: [
    [null, "2026-09-27T10:00:00Z", 8_703_180],
    ["First draft", "2026-09-28T10:00:00Z", 10_066_330],
    [null, "2026-10-02T10:00:00Z", 12_373_197],
    [null, "2026-10-07T10:00:00Z", 14_889_779],
    ["Client review", "2026-10-09T10:00:00Z", 18_769_510],
    [null, "2026-10-15T10:00:00Z", 18_979_225],
    ["Issued for tender", "2026-10-16T12:00:00Z", 19_293_798],
  ],
};

/** What the fake saw and how to steer it. */
export interface PanelApi {
  /** The input of every restore the page sent. */
  readonly restores: Record<string, unknown>[];
  /** The expected revision of every restore's envelope. */
  readonly revisions: string[];
  /** The input of every label edit the page sent. */
  readonly labelEdits: Record<string, unknown>[];
  /** The expected revision of every label edit's envelope. */
  readonly labelRevisions: string[];
  /** The query of every download the page sent. */
  readonly downloads: Record<string, string>[];
  /** The `Range` header of every ranged content read the page sent. */
  readonly contentReads: string[];
  /** Publish a version from "another writer": the object's head moves under the page. */
  moveHead(): void;
  /** Answer the details route with this status and code instead (an error state). */
  failDetails(status: number, code: string): void;
  /** Hold the details route until the returned function is called (the loading state). */
  holdDetails(): () => void;
  /** The File info and Provenance routes (facts, technical statements, history, records, export). */
  readonly provenance: ProvenanceApi;
}

/** The SHA-256 the inventory's fixity block records for the fixture (mockup 35). */
const SHA256 = `9b1f4e${"0".repeat(52)}c07a2d`;

/** The labels set after commit, by version; null clears one. */
type LabelOverrides = ReadonlyMap<string, string | null>;

/** The version records of a fixture, plus the versions later commits added, with later labels. */
function records(
  fixture: PanelFixture,
  extra: readonly Record<string, unknown>[],
  overrides: LabelOverrides,
) {
  const base = fixture.versions.map(([label, created], i) => ({
    version: `v${i + 1}`,
    created,
    message: null,
    user: { name: "Owner", address: null },
    label,
  }));
  return [...base, ...extra].map((record) => {
    const version = String(record.version);
    return overrides.has(version) ? { ...record, label: overrides.get(version) } : record;
  });
}

/**
 * The size of one version of the fixture file: its own for a fixture version; the newest
 * fixture version's for any version a restore or another commit added (a restore brings back
 * content of that size), and for no version at all.
 */
export function versionSize(fixture: PanelFixture, version?: string | null): number {
  const last = fixture.versions[fixture.versions.length - 1] as readonly [unknown, string, number];
  const asked = version ? fixture.versions[Number(version.slice(1)) - 1] : undefined;
  return (asked ?? last)[2];
}

/** The storage facts of the fixture at its head. */
function details(fixture: PanelFixture, count: number, version?: string | null) {
  const last = fixture.versions[fixture.versions.length - 1] as readonly [unknown, string, number];
  return {
    library_id: PANEL_LIBRARY,
    object_id: PANEL_OBJECT,
    version: `v${count}`,
    path: fixture.path,
    name: fixture.path.split("/").pop(),
    display_name: fixture.displayName,
    digest: "5a".repeat(64),
    digest_algorithm: "sha512",
    size: versionSize(fixture, version),
    fixity: { sha256: [SHA256] },
    last_change: { version: `v${count}`, created: last[1], message: null, user: null },
    type_hint: fixture.path.split(".").pop(),
    media_type: fixture.mediaType,
  };
}

/** The file's history, oldest first. */
function history(
  fixture: PanelFixture,
  extra: readonly Record<string, unknown>[],
  overrides: LabelOverrides,
  labelCommits: ReadonlySet<string>,
) {
  const changing = records(fixture, extra, overrides).filter(
    (version) => !labelCommits.has(String(version.version)),
  );
  const entries = changing.map((version, i) => ({
    version,
    change: i === 0 ? "added" : "changed",
    digest: `d${i}`,
    size: fixture.versions[i]?.[2] ?? fixture.versions[4]?.[2] ?? 0,
  }));
  return {
    library_id: PANEL_LIBRARY,
    object_id: PANEL_OBJECT,
    path: fixture.path,
    display_name: fixture.displayName,
    entries,
  };
}

/**
 * Serve the listing of the object holding `fixture`, and the file panel's routes for it.
 *
 * @param page - The page under test.
 * @param fixture - The file the panel shows.
 * @returns What the fake saw, and ways to fail or hold the details route.
 */
export async function servePanel(page: Page, fixture: PanelFixture): Promise<PanelApi> {
  const source = new MemoryFileSource({
    library_id: PANEL_LIBRARY,
    object_id: PANEL_OBJECT,
    files: [
      { path: fixture.path, displayName: fixture.displayName, content: new Uint8Array(PNG) },
      // A second file's original, named by the same file id with its own extension, so the
      // metadata routes (UI-F2.4) take it as a file's own original too (API-S-026).
      {
        path: `original/${PANEL_OBJECT.slice("urn:uuid:".length)}.txt`,
        displayName: "readme.txt",
        content: "hello",
      },
    ],
  });
  await serveFiles(page, source);
  const provenance = await serveProvenance(page, {
    displayName: fixture.displayName,
    path: fixture.path,
    mediaType: fixture.mediaType,
    size: versionSize(fixture),
  });
  const extra: Record<string, unknown>[] = [];
  const overrides = new Map<string, string | null>();
  const labelCommits = new Set<string>();
  const receipts = new Map<string, { input: string; answer: Record<string, unknown> }>();
  const fail: { status?: number; code?: string } = {};
  let hold: Promise<void> | undefined;
  const api: PanelApi = {
    restores: [],
    revisions: [],
    labelEdits: [],
    labelRevisions: [],
    downloads: [],
    contentReads: [],
    moveHead: () => {
      const version = `v${fixture.versions.length + extra.length + 1}`;
      labelCommits.add(version);
      extra.push({
        version,
        created: "2026-10-16T13:30:00Z",
        message: null,
        user: null,
        label: null,
      });
    },
    failDetails: (status, code) => Object.assign(fail, { status, code }),
    provenance,
    holdDetails: () => {
      let release: () => void = () => undefined;
      hold = new Promise<void>((resolve) => (release = resolve));
      return () => release();
    },
  };
  const json = (route: Route, body: unknown) => route.fulfill({ json: body });
  await page.route(/\/api\/v1\/files\/details\?/, async (route) => {
    await hold;
    if (fail.status) {
      const body = { code: fail.code, message: "refused", retryable: true };
      return route.fulfill({ status: fail.status, json: body });
    }
    const version = new URL(route.request().url()).searchParams.get("version");
    return json(route, details(fixture, fixture.versions.length + extra.length, version));
  });
  await page.route(/\/api\/v1\/files\/history\?/, (route) =>
    json(route, history(fixture, extra, overrides, labelCommits)),
  );
  await page.route(/\/api\/v1\/files\/versions\?/, (route) => {
    const versions = records(fixture, extra, overrides);
    const head = `v${versions.length}`;
    const body = { library_id: PANEL_LIBRARY, object_id: PANEL_OBJECT, head, versions };
    return json(route, { ...body, display_name: fixture.displayName });
  });
  await page.route(/\/api\/v1\/files\/history-size\?/, (route) =>
    json(route, {
      library_id: PANEL_LIBRARY,
      object_id: PANEL_OBJECT,
      path: fixture.path,
      versions: fixture.versions.length,
      contents: fixture.versions.length,
      bytes: 100_978_892,
    }),
  );
  await page.route("**/api/v1/files/restorations", async (route) => {
    const { envelope, input } = route.request().postDataJSON() as {
      envelope: { operation_id: string; expected_revision: string };
      input: Record<string, unknown>;
    };
    api.restores.push(input);
    api.revisions.push(envelope.expected_revision);
    const receipt = receipts.get(envelope.operation_id);
    if (receipt && receipt.input !== `restorations:${JSON.stringify(input)}`) {
      const body = { code: "invalid_request", message: "refused", retryable: false };
      return route.fulfill({ status: 400, json: body });
    }
    if (receipt) return json(route, receipt.answer);
    const head = `v${fixture.versions.length + extra.length}`;
    const operation = { operation_id: envelope.operation_id, error: null };
    if (envelope.expected_revision !== head) {
      const conflict = { ...operation, kind: "conflict", result: null };
      const refusal = { code: "conflict", message: "refused", retryable: false };
      return route.fulfill({ status: 409, json: { ...refusal, ...conflict } });
    }
    const version = `v${fixture.versions.length + extra.length + 1}`;
    const created = "2026-10-16T13:00:00Z";
    extra.push({ version, created, message: null, user: null, label: input.label ?? null });
    const result = { library_id: PANEL_LIBRARY, object_id: PANEL_OBJECT, version, created };
    const answer = { ...operation, kind: "committed", result };
    receipts.set(envelope.operation_id, { input: `restorations:${JSON.stringify(input)}`, answer });
    return json(route, answer);
  });
  await page.route(/\/api\/v1\/files\/download\?/, (route) => {
    api.downloads.push(Object.fromEntries(new URL(route.request().url()).searchParams));
    const name = encodeURIComponent(fixture.displayName);
    return route.fulfill({
      status: 200,
      body: "fixture bytes",
      headers: {
        "content-type": fixture.mediaType,
        "content-disposition": `attachment; filename*=UTF-8''${name}`,
      },
    });
  });
  await page.route("**/api/v1/files/labels", async (route) => {
    const { envelope, input } = route.request().postDataJSON() as {
      envelope: { operation_id: string; expected_revision: string };
      input: { version: string; label: string | null };
    };
    api.labelEdits.push(input);
    api.labelRevisions.push(envelope.expected_revision);
    const key = `labels:${JSON.stringify(input)}`;
    const receipt = receipts.get(envelope.operation_id);
    if (receipt && receipt.input !== key) {
      const body = { code: "invalid_request", message: "refused", retryable: false };
      return route.fulfill({ status: 400, json: body });
    }
    if (receipt) return json(route, receipt.answer);
    const head = `v${fixture.versions.length + extra.length}`;
    const current = records(fixture, extra, overrides).find((r) => r.version === input.version);
    const operation = { operation_id: envelope.operation_id, error: null };
    const refused =
      envelope.expected_revision !== head || !current || current.label === input.label;
    if (refused) {
      const refusal = { code: "conflict", message: "refused", retryable: false };
      return route.fulfill({ status: 409, json: { ...refusal, ...operation, kind: "conflict" } });
    }
    const version = `v${fixture.versions.length + extra.length + 1}`;
    const created = "2026-10-16T13:00:00Z";
    overrides.set(input.version, input.label);
    labelCommits.add(version);
    extra.push({ version, created, message: null, user: null, label: null });
    const result = { library_id: PANEL_LIBRARY, object_id: PANEL_OBJECT, version, created };
    const answer = { ...operation, kind: "committed", result };
    receipts.set(envelope.operation_id, { input: key, answer });
    return json(route, answer);
  });
  await page.route(/\/api\/v1\/files\/content\?/, (route) => {
    const url = new URL(route.request().url());
    const range = /^bytes=(\d+)-(\d+)$/.exec(route.request().headers().range ?? "");
    const total = versionSize(fixture, url.searchParams.get("version"));
    // Only a ranged read of a version above the desktop cap is this route's; the rest is the
    // listing fake's (previews).
    if (!range || total <= DESKTOP_ANSWER_CAP) return route.fallback();
    api.contentReads.push(route.request().headers().range as string);
    const [start, end] = [Number(range[1]), Number(range[2])];
    return route.fulfill({
      status: 206,
      body: Buffer.alloc(end - start + 1, 0x61),
      headers: {
        "content-type": "application/octet-stream",
        "content-range": `bytes ${start}-${end}/${total}`,
      },
    });
  });
  return api;
}
