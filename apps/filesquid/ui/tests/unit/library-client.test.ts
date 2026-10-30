/**
 * Unit tests for `ClientLibrarySource` (UI-1.1, UI-S-012, API-S-025, API-S-034, API-F2.19):
 * every call goes through `FileSquidClient` to the contract's route with the contract's body
 * (create, open, copy and rebuild as commands `{ envelope, input }` whose answer is an outcome
 * for the same operation id), a folder travels only as the opaque location ID, a `conflict`
 * outcome is the `ApiError` 409 the views word, and an answer that breaks the contract is a
 * `TypeError`.
 */
import { describe, expect, it } from "vitest";
import {
  ApiError,
  ClientLibrarySource,
  FileSquidClient,
  MAX_RETAINED_COMMANDS,
  type CommandEnvelope,
  type LibraryClient,
} from "../../src/index";
import type { CommandAnswer } from "../../src/api/commands";
import { CANDIDATES, COPY_REPORT, LIBRARY, REBUILD_JOB } from "../support/library-fakes";

/** One request as the fake fetch saw it. */
interface Seen {
  url: string;
  method: string;
  body: unknown;
}

/** How the fake answers a command: wrapped in an outcome of this kind for the sent id. */
type Wrap = "committed" | "pending" | "conflict" | "none";

/**
 * A client whose fetch answers `answer` (or a status/body pair) and records requests. A
 * command (a body with an `envelope`) is answered as an outcome of `wrap` for the operation
 * id it sent, with `answer` as its `result`; `none` answers `answer` as it is.
 */
function setup(answer: unknown, status = 200, wrap: Wrap = "committed") {
  const seen: Seen[] = [];
  const fetchFake = (async (input: RequestInfo | URL, init?: RequestInit) => {
    const text = typeof init?.body === "string" ? init.body : undefined;
    const body = text === undefined ? undefined : (JSON.parse(text) as Record<string, unknown>);
    seen.push({ url: String(input), method: init?.method ?? "GET", body });
    const envelope = body?.envelope as { operation_id: string } | undefined;
    const json =
      envelope === undefined || wrap === "none"
        ? answer
        : { operation_id: envelope.operation_id, kind: wrap, error: null, result: answer };
    return new Response(JSON.stringify(json), {
      status: wrap === "conflict" ? 409 : status,
      headers: { "Content-Type": "application/json" },
    });
  }) as typeof fetch;
  const client = new FileSquidClient({ baseUrl: "", fetch: fetchFake });
  return { source: new ClientLibrarySource(client), seen };
}

/** The envelope the fake saw, with the generated operation id left to the matcher. */
function envelopeOf(scope: string, expectedRevision: string) {
  return {
    schema_version: 1,
    operation_id: expect.any(String) as unknown as string,
    expected_revision: expectedRevision,
    scope,
  };
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

  it("creates and opens as commands scoped to the location, expecting revision 0, and sends no library_id", async () => {
    const created = setup(LIBRARY);
    await expect(created.source.create("loc-1")).resolves.toEqual(LIBRARY);
    expect(created.seen).toEqual([
      {
        url: "/api/v1/libraries",
        method: "POST",
        body: { envelope: envelopeOf("location:loc-1", "0"), input: { location_id: "loc-1" } },
      },
    ]);
    const opened = setup(LIBRARY);
    await opened.source.open("loc-2");
    expect(opened.seen).toEqual([
      {
        url: "/api/v1/libraries/openings",
        method: "POST",
        body: { envelope: envelopeOf("location:loc-2", "0"), input: { location_id: "loc-2" } },
      },
    ]);
  });

  it("copies as a command scoped to the library with the revision last read", async () => {
    const { source, seen } = setup(COPY_REPORT);
    await expect(source.copy("lib-1", "loc-3", "4")).resolves.toEqual(COPY_REPORT);
    expect(seen).toEqual([
      {
        url: "/api/v1/libraries/snapshot-copies",
        method: "POST",
        body: {
          envelope: envelopeOf("library:lib-1", "4"),
          input: { library_id: "lib-1", destination_location_id: "loc-3" },
        },
      },
    ]);
  });

  it("starts a rebuild as a command answered by its pending job, and reads the repair candidates", async () => {
    const rebuilt = setup(REBUILD_JOB, 200, "pending");
    await expect(rebuilt.source.rebuild("lib 1", "1")).resolves.toEqual(REBUILD_JOB);
    expect(rebuilt.seen).toEqual([
      {
        url: "/api/v1/libraries/rebuilds",
        method: "POST",
        body: { envelope: envelopeOf("library:lib 1", "1"), input: { library_id: "lib 1" } },
      },
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

  it("raises a conflict outcome as the ApiError 409 conflict the views word", async () => {
    const { source } = setup(null, 200, "conflict");
    const failure = await source.copy("lib-1", "loc-3", "1").catch((error: unknown) => error);
    expect(failure).toBeInstanceOf(ApiError);
    expect((failure as ApiError).status).toBe(409);
    expect((failure as ApiError).code).toBe("conflict");
  });

  it("rejects an outcome of another kind than the route answers, or one without a result", async () => {
    const pending = setup(LIBRARY, 200, "pending");
    await expect(pending.source.create("loc-1")).rejects.toBeInstanceOf(TypeError);
    const committed = setup(REBUILD_JOB, 200, "committed");
    await expect(committed.source.rebuild("lib-1", "1")).rejects.toBeInstanceOf(TypeError);
    const empty = setup(null);
    await expect(empty.source.open("loc-1")).rejects.toBeInstanceOf(TypeError);
    const plain = setup(LIBRARY, 200, "none");
    await expect(plain.source.create("loc-1")).rejects.toBeInstanceOf(TypeError);
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
    ["a library without an ID", { spec_version: "1.1", objects: 1, revision: "1" }],
    ["a library without a revision", { library_id: "a", spec_version: "1.1", objects: 1 }],
    ["a negative object count", { library_id: "a", spec_version: "1.1", objects: -1 }],
    ["a non-object answer", "nope"],
  ])("rejects %s from create with a TypeError", async (_name, answer) => {
    const { source } = setup(answer);
    await expect(source.create("loc-1")).rejects.toBeInstanceOf(TypeError);
  });

  it("rejects an invalid copied object without codes, over 1,000 candidates and a non-string provenance field", async () => {
    const invalid = [{ object_id: "o", error_codes: [] }];
    const copy = { objects: 1, files: 1, bytes: 1, invalid_objects: invalid };
    await expect(setup(copy).source.copy("a", "b", "1")).rejects.toBeInstanceOf(TypeError);
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

  it("checks a rebuild's pending job against the contract", async () => {
    const bad = [
      { ...REBUILD_JOB, outcome: "committed" },
      { ...REBUILD_JOB, job_id: 7 },
      { ...REBUILD_JOB, job: null },
      { ...REBUILD_JOB, job: { ...REBUILD_JOB.job, kind: 1 } },
      { ...REBUILD_JOB, scheduled: "yes" },
    ];
    for (const value of bad) {
      const pending = setup(value, 200, "pending");
      await expect(pending.source.rebuild("a", "1")).rejects.toBeInstanceOf(TypeError);
    }
  });

  it("builds a rebuild's job view again from its checked fields", async () => {
    const job = {
      ...REBUILD_JOB.job,
      state: "running",
      control: "pause",
      total: 9,
      error_code: "unavailable",
      lease_holder: "worker-1",
    };
    const pending = setup({ ...REBUILD_JOB, job }, 200, "pending");
    const answer = await pending.source.rebuild("a", "1");
    expect(answer.job).toEqual({
      ...REBUILD_JOB.job,
      state: "running",
      control: "pause",
      total: 9,
      error_code: "unavailable",
    });
  });

  it.each([
    ["no operation id", { operation_id: undefined }],
    ["an unknown state", { state: "sleeping" }],
    ["a non-boolean paused", { paused: "no" }],
    ["a negative done count", { done: -1 }],
    ["a fractional attempt count", { attempts: 1.5 }],
    ["a created time that is not RFC 3339", { created: "yesterday" }],
    ["no updated time", { updated: undefined }],
    ["an unknown control", { control: "stop" }],
    ["a negative total", { total: -2 }],
    ["an empty error code", { error_code: "" }],
    ["a malformed result", { result: { objects: "x" } }],
    ["no result", { result: undefined }],
  ])("rejects a rebuild job view with %s", async (_name, change) => {
    const job = { ...REBUILD_JOB.job, ...change };
    const pending = setup({ ...REBUILD_JOB, job }, 200, "pending");
    await expect(pending.source.rebuild("a", "1")).rejects.toBeInstanceOf(TypeError);
  });

  it("rejects malformed copy, rebuild, candidate and repair answers", async () => {
    await expect(setup({ objects: 1 }).source.copy("a", "b", "1")).rejects.toBeInstanceOf(
      TypeError,
    );
    const rebuilt = setup({ objects: "x" }, 200, "pending");
    await expect(rebuilt.source.rebuild("a", "1")).rejects.toBeInstanceOf(TypeError);
    await expect(setup({ candidates: 1 }).source.repairCandidates("a")).rejects.toBeInstanceOf(
      TypeError,
    );
    const request = { library_id: "a", object_ids: ["o"] };
    await expect(setup({ objects: [{}] }).source.repair(request)).rejects.toBeInstanceOf(TypeError);
  });
});

/** A library client whose commands answer from `answers` in order and record each envelope. */
function scripted(answers: (CommandAnswer | Error)[]) {
  const envelopes: CommandEnvelope[] = [];
  const client: LibraryClient = {
    request: () => Promise.reject(new Error("no request expected")),
    command: <T>(_path: string, envelope: CommandEnvelope) => {
      envelopes.push(envelope);
      const next = answers.shift() ?? new Error("no answer scripted");
      if (next instanceof Error) return Promise.reject(next);
      return Promise.resolve({ ...next, operation_id: envelope.operation_id } as CommandAnswer<T>);
    },
  };
  return { source: new ClientLibrarySource(client), envelopes };
}

/** A committed outcome carrying `result`; the operation id is the one sent. */
function committed(result: unknown): CommandAnswer {
  return { operation_id: "", kind: "committed", error: null, result };
}

describe("ClientLibrarySource retries", () => {
  it("resends the same operation id when a create's answer was lost, then releases it", async () => {
    const lost = new TypeError("Failed to fetch");
    const { source, envelopes } = scripted([lost, committed(LIBRARY), committed(LIBRARY)]);
    await expect(source.create("loc-1")).rejects.toBe(lost);
    await expect(source.create("loc-1")).resolves.toEqual(LIBRARY);
    await source.create("loc-1");
    const ids = envelopes.map((envelope) => envelope.operation_id);
    expect(ids[1]).toBe(ids[0]);
    expect(ids[2]).not.toBe(ids[0]);
  });

  it("keeps the operation id of a copy refused with 503 or a conflict outcome", async () => {
    const outage = new ApiError(503, "/api/v1/libraries/snapshot-copies", { retryable: true });
    const conflict: CommandAnswer = { operation_id: "", kind: "conflict", error: null };
    const { source, envelopes } = scripted([outage, conflict, committed(COPY_REPORT)]);
    await expect(source.copy("lib-1", "loc-2", "1")).rejects.toBe(outage);
    await expect(source.copy("lib-1", "loc-2", "1")).rejects.toBeInstanceOf(ApiError);
    await expect(source.copy("lib-1", "loc-2", "1")).resolves.toEqual(COPY_REPORT);
    expect(new Set(envelopes.map((envelope) => envelope.operation_id)).size).toBe(1);
  });

  it("sends a fresh operation id for another input or another expected revision", async () => {
    const lost = new TypeError("Failed to fetch");
    const { source, envelopes } = scripted([lost, lost, lost]);
    await expect(source.copy("lib-1", "loc-2", "1")).rejects.toBe(lost);
    await expect(source.copy("lib-1", "loc-3", "1")).rejects.toBe(lost);
    await expect(source.copy("lib-1", "loc-2", "2")).rejects.toBe(lost);
    expect(new Set(envelopes.map((envelope) => envelope.operation_id)).size).toBe(3);
  });

  it(`keeps at most ${MAX_RETAINED_COMMANDS} unanswered commands, dropping the oldest`, async () => {
    const lost = new TypeError("Failed to fetch");
    const count = MAX_RETAINED_COMMANDS + 1;
    const { source, envelopes } = scripted(Array.from({ length: count + 2 }, () => lost));
    for (let i = 0; i < count; i += 1) {
      await expect(source.open(`loc-${i}`)).rejects.toBe(lost);
    }
    await expect(source.open("loc-0")).rejects.toBe(lost);
    await expect(source.open(`loc-${count - 1}`)).rejects.toBe(lost);
    const ids = envelopes.map((envelope) => envelope.operation_id);
    expect(ids[count]).not.toBe(ids[0]);
    expect(ids[count + 1]).toBe(ids[count - 1]);
  });
});
