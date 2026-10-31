/**
 * Unit tests for the model of the Provenance tab (UI-F2.15; UI-S-020, UI-S-031 mockup 36;
 * API-S-029; ADR 040): the history entries newest first, the agents classified by their
 * identifiers (installation, host software with its version, tool, unrecorded), the
 * `filesquid-prov/1` record parsed with bounds, the `prov:` attribute rows of one activity,
 * the kind words of every operation kind, and the paging of the history route with its bounds.
 */
import { describe, expect, it, vi } from "vitest";
import labels from "../../src/theme/labels.en.json";
import {
  MAX_HISTORY_PAGES,
  OPERATION_KINDS,
  activityAttributes,
  activityRow,
  activityTime,
  activityViews,
  classifyAgent,
  kindWords,
  parseProvRecord,
  readAllHistory,
  type ActivityView,
  type HistoryEntry,
  type HistoryPage,
} from "../../src/files/provenance";

const OBJECT = "urn:uuid:7f3c9a1e-0b2c-4d5e-8f90-a1b2c341d2b8";
const INSTALLATION = "urn:uuid:5a0c41e2-1111-4222-8333-44445555e19b";
const HOST = "urn:filesquid:software:FileSquid:desktop:0.1.0";

/** One history entry; every member can be overridden. */
function entry(version: number, over: Partial<HistoryEntry> = {}): HistoryEntry {
  return {
    kind: "metadata.edit",
    operation_id: `op-${version}`,
    version: { object_id: OBJECT, version: `v${version}` },
    agents: [INSTALLATION, HOST],
    started_at: `2026-10-1${version}T11:05:02Z`,
    ended_at: `2026-10-1${version}T11:05:02Z`,
    journal: { operation_id: `op-${version}`, journal_entry_ids: ["7"] },
    label: null,
    reconstructed: false,
    ...over,
  };
}

/** A `filesquid-prov/1` record of version 4 that derives v3 and used v3 (mockup 36). */
function record(): unknown {
  const path = "Harbour project/facade-study.jpg";
  const sha = `9b1f4e${"0".repeat(52)}c07a2d`;
  return {
    "@context": {},
    "fsq:profile": "filesquid-prov/1",
    "@graph": [
      {
        "@id": `urn:filesquid:activity:op-4:${OBJECT}:v4`,
        "@type": "prov:Activity",
        "fsq:operation": "op-4",
        "fsq:kind": "metadata.edit",
        "fsq:objectVersion": { "fsq:objectId": OBJECT, "fsq:version": "v4" },
        "prov:startedAtTime": { "@type": "xsd:dateTime", "@value": "2026-10-16T11:05:02Z" },
        "prov:endedAtTime": { "@type": "xsd:dateTime", "@value": "2026-10-16T11:05:02Z" },
        "prov:used": [{ "@id": `urn:filesquid:entity:${OBJECT}:v3:${encodeURIComponent(path)}` }],
        "prov:wasAssociatedWith": [{ "@id": INSTALLATION }, { "@id": HOST }],
      },
      {
        "@id": `urn:filesquid:entity:${OBJECT}:v3:${encodeURIComponent(path)}`,
        "@type": "prov:Entity",
        "fsq:objectId": OBJECT,
        "fsq:version": "v3",
        "fsq:path": path,
        "fsq:sha256": sha,
      },
      {
        "@id": `urn:filesquid:entity:${OBJECT}:v4:${encodeURIComponent(path)}`,
        "@type": "prov:Entity",
        "fsq:objectId": OBJECT,
        "fsq:version": "v4",
        "fsq:path": path,
        "fsq:sha256": sha,
        "prov:wasGeneratedBy": { "@id": `urn:filesquid:activity:op-4:${OBJECT}:v4` },
        "prov:wasDerivedFrom": [
          { "@id": `urn:filesquid:entity:${OBJECT}:v3:${encodeURIComponent(path)}` },
        ],
      },
      {
        "@id": `urn:filesquid:entity:${OBJECT}:v1:old`,
        "@type": "prov:Entity",
        "fsq:objectId": OBJECT,
        "fsq:version": "v1",
        "fsq:path": "old",
        "fsq:sha256": sha,
        "prov:wasInvalidatedBy": { "@id": `urn:filesquid:activity:op-4:${OBJECT}:v4` },
      },
    ],
  };
}

describe("classifyAgent", () => {
  it("names the installation agent by its urn:uuid", () => {
    expect(classifyAgent(INSTALLATION)).toEqual({ id: INSTALLATION, kind: "installation" });
  });

  it("reads the product and version of the host software agent from its identifier", () => {
    expect(classifyAgent("urn:filesquid:software:FileSquid:desktop:0.1.0")).toEqual({
      id: "urn:filesquid:software:FileSquid:desktop:0.1.0",
      kind: "software",
      product: "FileSquid",
      version: "0.1.0",
    });
  });

  it("decodes the percent-encoded parts of a software or tool agent", () => {
    expect(classifyAgent("urn:filesquid:software:File%20Squid:desktop:1.0%2Bx")).toMatchObject({
      product: "File Squid",
      version: "1.0+x",
    });
    expect(classifyAgent("urn:filesquid:tool:Resizer:2.4")).toMatchObject({
      kind: "tool",
      name: "Resizer",
      version: "2.4",
    });
  });

  it("marks an unrecorded agent with its role", () => {
    expect(classifyAgent("urn:filesquid:agent:unrecorded:installation").kind).toBe(
      "unrecorded-installation",
    );
    expect(classifyAgent("urn:filesquid:agent:unrecorded:host").kind).toBe("unrecorded-host");
  });

  it("keeps any other identifier as an agent, never throws", () => {
    expect(classifyAgent("urn:uuid:not-a-uuid")).toEqual({
      id: "urn:uuid:not-a-uuid",
      kind: "other",
    });
    expect(classifyAgent("urn:filesquid:software:broken")).toMatchObject({ kind: "other" });
  });
});

describe("activityViews", () => {
  it("lists the activities newest first, the history route answers oldest first", () => {
    const views = activityViews([entry(1), entry(2), entry(3)]);
    expect(views.map((v) => v.version)).toEqual(["v3", "v2", "v1"]);
    expect(views[0]).toMatchObject({
      key: "v3:op-3",
      rawKind: "metadata.edit",
      operationId: "op-3",
      reconstructed: false,
      startedAt: "2026-10-13T11:05:02Z",
    });
    expect(views[0]?.agents.map((a) => a.kind)).toEqual(["installation", "software"]);
  });

  it("keeps a reconstructed activity's null start and marks it", () => {
    const [view] = activityViews([
      entry(1, {
        reconstructed: true,
        started_at: null,
        agents: ["urn:filesquid:agent:unrecorded:installation"],
      }),
    ]);
    expect(view).toMatchObject({ reconstructed: true, startedAt: null });
    expect(view?.agents[0]?.kind).toBe("unrecorded-installation");
  });

  it("gives two activities of one operation their own keys", () => {
    const views = activityViews([
      entry(1, { operation_id: "same" }),
      entry(2, { operation_id: "same" }),
    ]);
    expect(new Set(views.map((v) => v.key)).size).toBe(2);
  });
});

describe("kindWords", () => {
  it("has words for every operation kind of the contract, in the label table", () => {
    expect(OPERATION_KINDS).toHaveLength(22);
    for (const kind of OPERATION_KINDS) expect(kindWords(kind, labels).length).toBeGreaterThan(0);
    expect(kindWords("metadata.edit", labels)).toBe("metadata edit");
    expect(kindWords("delete", labels)).toBe("remove");
    expect(kindWords("revert", labels)).toBe("restore");
  });
});

describe("activityTime", () => {
  it("writes a UTC time as a short date and a 24-hour clock with UTC", () => {
    expect(activityTime("2026-10-16T11:05:02Z", "en")).toBe("Oct 16, 2026, 11:05 UTC");
  });

  it("returns an unreadable time as it came", () => {
    expect(activityTime("soon", "en")).toBe("soon");
  });
});

describe("parseProvRecord", () => {
  it("reads the used, generated, derived and invalidated entities of the activity", () => {
    const parsed = parseProvRecord(record());
    expect(parsed.used.map((e) => `${e.version} ${e.path}`)).toEqual([
      "v3 Harbour project/facade-study.jpg",
    ]);
    expect(parsed.generated).toHaveLength(1);
    expect(parsed.generated[0]?.entity.version).toBe("v4");
    expect(parsed.generated[0]?.derivedFrom.map((e) => e.version)).toEqual(["v3"]);
    expect(parsed.invalidated.map((e) => e.version)).toEqual(["v1"]);
  });

  it("refuses a record of another profile and a non-object", () => {
    expect(() => parseProvRecord({ "fsq:profile": "filesquid-prov/2", "@graph": [] })).toThrow(
      TypeError,
    );
    expect(() => parseProvRecord("text")).toThrow(TypeError);
    expect(() => parseProvRecord({ "fsq:profile": "filesquid-prov/1" })).toThrow(TypeError);
  });

  it("names a derived entity that the graph does not hold from its identifier", () => {
    const doc = record() as { "@graph": Record<string, unknown>[] };
    const generated = doc["@graph"][2] as Record<string, unknown>;
    generated["prov:wasDerivedFrom"] = [{ "@id": `urn:filesquid:entity:${OBJECT}:v2:a%2Fb.txt` }];
    const parsed = parseProvRecord(doc);
    expect(parsed.generated[0]?.derivedFrom[0]).toMatchObject({
      objectId: OBJECT,
      version: "v2",
      path: "a/b.txt",
    });
  });

  it("bounds the graph it reads", () => {
    const doc = record() as { "@graph": unknown[] };
    for (let i = 0; i < 5000; i += 1)
      doc["@graph"].push({ "@id": `x${i}`, "@type": "prov:Entity" });
    expect(() => parseProvRecord(doc)).toThrow(TypeError);
  });
});

describe("activityAttributes", () => {
  const [view] = activityViews([
    entry(4, { started_at: "2026-10-16T11:05:02Z", ended_at: "2026-10-16T11:05:02Z" }),
  ]);

  it("lists the prov attributes of the mockup, then the relations of the record", () => {
    const rows = activityAttributes(view as ActivityView, parseProvRecord(record()), labels, "en");
    expect(rows.map((r) => r.name)).toEqual([
      "prov:startedAtTime",
      "prov:endedAtTime",
      "prov:wasAssociatedWith",
      "prov:used",
      "prov:wasGeneratedBy (entity v4)",
      "prov:wasDerivedFrom",
      "prov:wasInvalidatedBy",
    ]);
    const agents = rows[2];
    expect(agents?.values.map((v) => v.text)).toEqual([
      "urn:uuid:5a0c41e2-…-55e19b",
      "owner agent · this installation",
      "FileSquid 0.1.0",
      "prov:SoftwareAgent",
    ]);
    expect(rows[3]?.values.map((v) => v.text)).toEqual([
      "urn:uuid:7f3c9a1e-…-41d2b8@v3",
      "Harbour project/facade-study.jpg",
      "sha256 9b1f4e…c07a2d",
    ]);
    expect(rows[5]?.values[0]?.text).toBe("…41d2b8@v3");
  });

  it("shows the times as the record gives them, and what was not recorded as such", () => {
    const [bare] = activityViews([
      entry(1, {
        reconstructed: true,
        started_at: null,
        agents: ["urn:filesquid:agent:unrecorded:host"],
      }),
    ]);
    const rows = activityAttributes(bare as ActivityView, undefined, labels, "en");
    expect(rows[0]).toMatchObject({ name: "prov:startedAtTime" });
    expect(rows[0]?.values[0]?.text).toBe("not recorded");
    expect(rows[2]?.values.map((v) => v.text)).toEqual(["software agent · unrecorded"]);
  });

  it("gives every copyable value its whole text", () => {
    const rows = activityAttributes(view as ActivityView, parseProvRecord(record()), labels, "en");
    expect(rows[2]?.values[0]?.copy).toBe(INSTALLATION);
    expect(rows[3]?.values[2]?.copy).toBe(`9b1f4e${"0".repeat(52)}c07a2d`);
  });
});

describe("activityRow", () => {
  it("makes one searchable row per activity that matches its kind, version, time and operation", () => {
    const [view] = activityViews([entry(2)]);
    const row = activityRow(view as ActivityView, labels, "en");
    expect(row).toMatchObject({ key: "v2:op-2", label: "metadata edit", value: "v2" });
    for (const word of ["metadata", "v2", "op-2", "2026-10-12", "FileSquid"]) {
      expect(row.searchText?.toLowerCase()).toContain(word.toLowerCase());
    }
  });
});

describe("readAllHistory", () => {
  const page = (items: HistoryEntry[], next: string | null, truncated = false): HistoryPage => ({
    items,
    next_cursor: next,
    truncated,
  });

  it("follows the cursor to the last page and joins the entries oldest first", async () => {
    const history = vi
      .fn()
      .mockResolvedValueOnce(page([entry(1)], "c1"))
      .mockResolvedValueOnce(page([entry(2)], null));
    const all = await readAllHistory({ history }, FILE_REF, {});
    expect(all.entries.map((e) => e.version.version)).toEqual(["v1", "v2"]);
    expect(all).toMatchObject({ capped: false, truncated: false });
    expect(history.mock.calls[1]?.[1]).toMatchObject({ cursor: "c1" });
  });

  it("stops at the page bound and says so", async () => {
    let n = 0;
    const history = vi.fn(async () => page([entry(1)], `more-${(n += 1)}`));
    const all = await readAllHistory({ history }, FILE_REF, {});
    expect(history).toHaveBeenCalledTimes(MAX_HISTORY_PAGES);
    expect(all.capped).toBe(true);
  });

  it("carries the index's truncated flag", async () => {
    const history = vi.fn(async () => page([entry(1)], null, true));
    expect((await readAllHistory({ history }, FILE_REF, {})).truncated).toBe(true);
  });

  it("stops when the signal aborts", async () => {
    const controller = new AbortController();
    const history = vi.fn(async () => {
      controller.abort();
      return page([entry(1)], "more");
    });
    await expect(
      readAllHistory({ history }, FILE_REF, { signal: controller.signal }),
    ).rejects.toThrow();
    expect(history).toHaveBeenCalledTimes(1);
  });
});

const FILE_REF = {
  library_id: "lib",
  object_id: OBJECT,
  version: "v4",
  path: `original/${OBJECT.slice("urn:uuid:".length)}.jpg`,
  display_name: "facade-study.jpg",
};
