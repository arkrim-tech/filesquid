/**
 * Unit tests for `ClientLibrarySource` (UI-1.1, UI-S-012, API-S-025, API-S-034): every call goes
 * through `FileSquidClient` to the contract's route with the contract's body, a folder travels
 * only as the opaque location ID, and an answer that breaks the contract is a `TypeError`.
 */
import { describe, expect, it } from "vitest";
import { ApiError, ClientLibrarySource, FileSquidClient } from "../../src/index";
import { CANDIDATES, COPY_REPORT, LIBRARY, REBUILD } from "../support/library-fakes";

/** One request as the fake fetch saw it. */
interface Seen {
  url: string;
  method: string;
  body: unknown;
}

/** A client whose fetch answers `answer` (or a status/body pair) and records requests. */
function setup(answer: unknown, status = 200) {
  const seen: Seen[] = [];
  const fetchFake = (async (input: RequestInfo | URL, init?: RequestInit) => {
    const text = typeof init?.body === "string" ? init.body : undefined;
    seen.push({
      url: String(input),
      method: init?.method ?? "GET",
      body: text === undefined ? undefined : (JSON.parse(text) as unknown),
    });
    return new Response(JSON.stringify(answer), {
      status,
      headers: { "Content-Type": "application/json" },
    });
  }) as typeof fetch;
  const client = new FileSquidClient({ baseUrl: "", fetch: fetchFake });
  return { source: new ClientLibrarySource(client), seen };
}

describe("ClientLibrarySource", () => {
  it.each(["create", "open", "copy_destination"] as const)(
    "asks the location-choices route for purpose %s",
    async (purpose) => {
      const { source, seen } = setup({ outcome: "chosen", location_id: "loc-9" });
      await expect(source.chooseLocation(purpose)).resolves.toEqual({
        outcome: "chosen",
        location_id: "loc-9",
      });
      expect(seen).toEqual([
        { url: "/api/v1/libraries/location-choices", method: "POST", body: { purpose } },
      ]);
    },
  );

  it("answers cancelled without a location ID", async () => {
    const { source } = setup({ outcome: "cancelled" });
    await expect(source.chooseLocation("open")).resolves.toEqual({ outcome: "cancelled" });
  });

  it("passes the location ID as location_id to create and to open, and sends no library_id", async () => {
    const created = setup(LIBRARY);
    await expect(created.source.create("loc-1")).resolves.toEqual(LIBRARY);
    expect(created.seen).toEqual([
      { url: "/api/v1/libraries", method: "POST", body: { location_id: "loc-1" } },
    ]);
    const opened = setup(LIBRARY);
    await opened.source.open("loc-2");
    expect(opened.seen).toEqual([
      { url: "/api/v1/libraries/openings", method: "POST", body: { location_id: "loc-2" } },
    ]);
  });

  it("passes the destination as destination_location_id with the library to snapshot-copies", async () => {
    const { source, seen } = setup(COPY_REPORT);
    await expect(source.copy("lib-1", "loc-3")).resolves.toEqual(COPY_REPORT);
    expect(seen).toEqual([
      {
        url: "/api/v1/libraries/snapshot-copies",
        method: "POST",
        body: { library_id: "lib-1", destination_location_id: "loc-3" },
      },
    ]);
  });

  it("rebuilds and reads the repair candidates of one library", async () => {
    const rebuilt = setup(REBUILD);
    await expect(rebuilt.source.rebuild("lib 1")).resolves.toEqual(REBUILD);
    expect(rebuilt.seen).toEqual([
      { url: "/api/v1/libraries/rebuilds", method: "POST", body: { library_id: "lib 1" } },
    ]);
    const candidates = setup(CANDIDATES);
    await expect(candidates.source.repairCandidates("lib 1")).resolves.toEqual(CANDIDATES);
    expect(candidates.seen).toEqual([
      {
        url: "/api/v1/libraries/repair-candidates?library_id=lib%201",
        method: "GET",
        body: undefined,
      },
    ]);
  });

  it("posts the repair request as given", async () => {
    const answer = {
      library_id: "lib-1",
      run_id: "r",
      objects: [
        { object_id: "o", version: "v2", result: "repaired", error_code: null, retryable: false },
      ],
    };
    const { source, seen } = setup(answer);
    const request = {
      library_id: "lib-1",
      object_ids: ["o"],
      consent: {
        library_id: "lib-1",
        object_ids: ["o"],
        statement_id: "version_inventory_repair.v2",
        confirmed: true,
      },
    };
    await expect(source.repair(request)).resolves.toEqual(answer);
    expect(seen).toEqual([{ url: "/api/v1/libraries/repairs", method: "POST", body: request }]);
  });

  it("lets an API refusal through unchanged", async () => {
    const { source } = setup({ code: "unavailable", message: "x", retryable: true }, 503);
    const failure = await source.chooseLocation("create").catch((error: unknown) => error);
    expect(failure).toBeInstanceOf(ApiError);
    expect((failure as ApiError).status).toBe(503);
    expect((failure as ApiError).code).toBe("unavailable");
  });

  it.each([
    ["a chosen answer without a location ID", { outcome: "chosen" }],
    ["a location ID over 128 bytes", { outcome: "chosen", location_id: "x".repeat(129) }],
    ["an unknown outcome", { outcome: "picked", location_id: "a" }],
  ])("rejects %s with a TypeError", async (_name, answer) => {
    const { source } = setup(answer);
    await expect(source.chooseLocation("open")).rejects.toBeInstanceOf(TypeError);
  });

  it.each([
    ["a library without an ID", { spec_version: "1.1", objects: 1 }],
    ["a negative object count", { library_id: "a", spec_version: "1.1", objects: -1 }],
    ["a non-object answer", "nope"],
  ])("rejects %s from create with a TypeError", async (_name, answer) => {
    const { source } = setup(answer);
    await expect(source.create("loc-1")).rejects.toBeInstanceOf(TypeError);
  });

  it("rejects an invalid copied object without codes, over 1,000 candidates and a non-string provenance field", async () => {
    const invalid = [{ object_id: "o", error_codes: [] }];
    const copy = { objects: 1, files: 1, bytes: 1, invalid_objects: invalid };
    await expect(setup(copy).source.copy("a", "b")).rejects.toBeInstanceOf(TypeError);
    const one = CANDIDATES.candidates[0];
    const many = { candidates: Array.from({ length: 1001 }, () => one), truncated: true };
    await expect(setup(many).source.repairCandidates("a")).rejects.toBeInstanceOf(TypeError);
    const object = {
      object_id: "o",
      version: "v2",
      result: "repaired",
      error_code: null,
      retryable: false,
    };
    const answer = {
      library_id: "a",
      run_id: "r",
      objects: [{ ...object, provenance_version: 2 }],
    };
    const request = { library_id: "a", object_ids: ["o"] };
    await expect(setup(answer).source.repair(request)).rejects.toBeInstanceOf(TypeError);
  });

  it("checks a rebuild's optional journal restore against the contract", async () => {
    const journal = { restored: 2, already_present: 1, conflicting: 0, unreadable: 0 };
    const sealed = { ...journal, last_sealed_at: "2026-10-20T12:00:00Z" };
    for (const good of [journal, sealed]) {
      const report = { ...REBUILD, journal: good };
      await expect(setup(report).source.rebuild("a")).resolves.toEqual(report);
    }
    const extra = { ...REBUILD, objects: [{ ...REBUILD.objects[0], path: "/x" }], path: "/x" };
    await expect(setup(extra).source.rebuild("a")).resolves.toStrictEqual({
      objects: [REBUILD.objects[0]],
      unreadable: REBUILD.unreadable,
    });
    const bad = [{ ...journal, restored: -1 }, { ...journal, conflicting: "0" }, null, []];
    const dates = ["yesterday", "2026-10-20", "2026-13-40T99:00:00Z", "2026-10-20T12:00:00"];
    const undated = dates.map((last_sealed_at) => ({ ...journal, last_sealed_at }));
    for (const value of [...bad, ...undated, { ...journal, last_sealed_at: 5 }]) {
      const report = { ...REBUILD, journal: value };
      await expect(setup(report).source.rebuild("a")).rejects.toBeInstanceOf(TypeError);
    }
  });

  it("rejects malformed copy, rebuild, candidate and repair answers", async () => {
    await expect(setup({ objects: 1 }).source.copy("a", "b")).rejects.toBeInstanceOf(TypeError);
    await expect(setup({ objects: "x" }).source.rebuild("a")).rejects.toBeInstanceOf(TypeError);
    await expect(setup({ candidates: 1 }).source.repairCandidates("a")).rejects.toBeInstanceOf(
      TypeError,
    );
    const request = { library_id: "a", object_ids: ["o"] };
    await expect(setup({ objects: [{}] }).source.repair(request)).rejects.toBeInstanceOf(TypeError);
  });
});
