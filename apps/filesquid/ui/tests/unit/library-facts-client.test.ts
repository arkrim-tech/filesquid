/**
 * Unit tests for the library routes UI-1.3 adds to `ClientLibrarySource` (API-S-038 facts and
 * recent libraries, a named create, the first collection) and for `ClientRebuildJobSource` (API-S-037
 * job read, pause, resume, cancel and the `job.update` watch): every call goes through
 * `FileSquidClient` to the contract's route, the controls send no body, and an answer that
 * breaks the contract is a `TypeError`.
 */
import { describe, expect, it, vi } from "vitest";
import {
  ApiError,
  ClientRebuildJobSource,
  ClientLibrarySource,
  FileSquidClient,
  type ApiEvent,
} from "../../src/index";
import { LIBRARY, REBUILD_JOB } from "../support/library-fakes";

/** One request as the fake fetch saw it. */
interface Seen {
  url: string;
  method: string;
  body: unknown;
}

/** A client whose fetch answers `answer` for every route (a command's result wrapped). */
function setup(answer: unknown, status = 200, committed = true) {
  const seen: Seen[] = [];
  const fetchFake = (async (input: RequestInfo | URL, init?: RequestInit) => {
    const text = typeof init?.body === "string" ? init.body : undefined;
    const body = text === undefined ? undefined : (JSON.parse(text) as Record<string, unknown>);
    seen.push({ url: String(input), method: init?.method ?? "GET", body });
    const envelope = body?.envelope as { operation_id: string } | undefined;
    const json =
      envelope === undefined || !committed
        ? answer
        : { operation_id: envelope.operation_id, kind: "committed", error: null, result: answer };
    return new Response(JSON.stringify(json), {
      status,
      headers: { "Content-Type": "application/json" },
    });
  }) as typeof fetch;
  const client = new FileSquidClient({ baseUrl: "", fetch: fetchFake });
  return { client, seen };
}

const FACTS = {
  library_id: "lib-1",
  name: "Family archive",
  free_bytes: 5_000,
  free_bytes_reason: null,
  revision: "1",
};

describe("ClientLibrarySource library facts and recent libraries", () => {
  it("reads the facts of one library from its route", async () => {
    const { client, seen } = setup(FACTS);
    await expect(new ClientLibrarySource(client).facts("lib 1")).resolves.toEqual(FACTS);
    expect(seen).toEqual([
      { url: "/api/v1/libraries/lib%201/facts", method: "GET", body: undefined },
    ]);
  });

  it("lists the recent libraries", async () => {
    const recent = {
      libraries: [
        {
          library_id: "lib-1",
          name: "Family archive",
          last_opened: "2026-10-31T09:30:00Z",
          open: true,
          free_bytes: 10,
          free_bytes_reason: null,
        },
      ],
    };
    const { client, seen } = setup(recent);
    await expect(new ClientLibrarySource(client).recent()).resolves.toEqual(recent);
    expect(seen[0]).toMatchObject({ url: "/api/v1/libraries/recent", method: "GET" });
  });

  it("passes a 503 of a host without a registry through as the ApiError", async () => {
    const { client } = setup({ code: "unavailable", retryable: true }, 503);
    await expect(new ClientLibrarySource(client).recent()).rejects.toBeInstanceOf(ApiError);
  });

  it("refuses facts that break the contract", async () => {
    const { client } = setup({ ...FACTS, free_bytes: "lots" });
    await expect(new ClientLibrarySource(client).facts("lib-1")).rejects.toBeInstanceOf(TypeError);
  });

  it("still reads the revision from the facts", async () => {
    const { client } = setup({ ...FACTS, revision: "4" });
    await expect(new ClientLibrarySource(client).revision("lib-1")).resolves.toBe("4");
  });
});

describe("ClientLibrarySource named create and first collection", () => {
  it("sends the library name with the location when one is given", async () => {
    const { client, seen } = setup(LIBRARY);
    await new ClientLibrarySource(client).create("loc-1", "Family archive");
    expect(seen[0]?.body).toMatchObject({
      envelope: { scope: "location:loc-1", expected_revision: "0" },
      input: { location_id: "loc-1", name: "Family archive" },
    });
  });

  it("sends no name field when none is given", async () => {
    const { client, seen } = setup(LIBRARY);
    await new ClientLibrarySource(client).create("loc-1");
    expect((seen[0]?.body as { input: object }).input).toEqual({ location_id: "loc-1" });
  });

  it("creates the first collection as a command scoped to the library at its revision", async () => {
    const collection = { collection_id: "col-1", title: "untitled collection" };
    const { client, seen } = setup(collection);
    const made = await new ClientLibrarySource(client).createCollection(
      "lib-1",
      "untitled collection",
      "1",
    );
    expect(made).toEqual(collection);
    expect(seen[0]).toMatchObject({
      url: "/api/v1/libraries/collections",
      method: "POST",
      body: {
        envelope: { scope: "library:lib-1", expected_revision: "1" },
        input: { library_id: "lib-1", title: "untitled collection" },
      },
    });
  });

  it("keeps the operation id of a collection command that did not answer, so a retry is not a second collection", async () => {
    let calls = 0;
    const ids: string[] = [];
    const fetchFake = (async (_input: RequestInfo | URL, init?: RequestInit) => {
      const body = JSON.parse(String(init?.body)) as { envelope: { operation_id: string } };
      ids.push(body.envelope.operation_id);
      calls += 1;
      if (calls === 1) {
        return new Response(JSON.stringify({ code: "internal", retryable: true }), {
          status: 500,
          headers: { "Content-Type": "application/json" },
        });
      }
      return new Response(
        JSON.stringify({
          operation_id: body.envelope.operation_id,
          kind: "committed",
          error: null,
          result: { collection_id: "col-1", title: "untitled collection" },
        }),
        { status: 200, headers: { "Content-Type": "application/json" } },
      );
    }) as typeof fetch;
    const source = new ClientLibrarySource(new FileSquidClient({ baseUrl: "", fetch: fetchFake }));
    await expect(source.createCollection("lib-1", "untitled collection", "1")).rejects.toThrow();
    await source.createCollection("lib-1", "untitled collection", "1");
    expect(ids).toHaveLength(2);
    expect(ids[1]).toBe(ids[0]);
  });

  it("refuses a collection answer that breaks the contract", async () => {
    const { client } = setup({ title: "untitled collection" });
    const source = new ClientLibrarySource(client);
    await expect(
      source.createCollection("lib-1", "untitled collection", "1"),
    ).rejects.toBeInstanceOf(TypeError);
  });
});

describe("ClientRebuildJobSource", () => {
  const job = REBUILD_JOB.job;

  it("reads one job from its route", async () => {
    const { client, seen } = setup(job);
    await expect(new ClientRebuildJobSource(client).job("job-1")).resolves.toEqual(job);
    expect(seen[0]).toMatchObject({ url: "/api/v1/jobs/job-1", method: "GET" });
  });

  it.each([
    ["pause", "paused", true],
    ["resume", "running", false],
    ["cancel", "cancelled", false],
  ] as const)(
    "%s posts to its route with no body and answers the job view",
    async (name, state, paused) => {
      const answered = { ...job, state: state === "paused" ? "running" : state, paused };
      const { client, seen } = setup({ job: answered, scheduled: true }, 200, false);
      const source = new ClientRebuildJobSource(client);
      await expect(source[name]("job-1")).resolves.toEqual(answered);
      expect(seen).toEqual([
        { url: `/api/v1/jobs/job-1/${name}`, method: "POST", body: undefined },
      ]);
    },
  );

  it("escapes a job id in the route", async () => {
    const { client, seen } = setup(job);
    await new ClientRebuildJobSource(client).job("a/b");
    expect(seen[0]?.url).toBe("/api/v1/jobs/a%2Fb");
  });

  it("refuses a job that breaks the contract", async () => {
    const { client } = setup({ ...job, state: "sleeping" });
    await expect(new ClientRebuildJobSource(client).job("job-1")).rejects.toBeInstanceOf(TypeError);
  });

  it("passes the 409 of a refused control through as the ApiError", async () => {
    const { client } = setup({ code: "conflict", retryable: false }, 409);
    await expect(new ClientRebuildJobSource(client).pause("job-1")).rejects.toBeInstanceOf(
      ApiError,
    );
  });
});

describe("ClientRebuildJobSource.watch", () => {
  /** A client whose event channel the test drives. */
  function channel() {
    let listener: ((event: ApiEvent) => void) | undefined;
    let opened: (() => void) | undefined;
    const close = vi.fn();
    const client = {
      request: vi.fn(),
      subscribe: (next: (event: ApiEvent) => void, onOpen?: () => void) => {
        listener = next;
        opened = onOpen;
        return close;
      },
    };
    return {
      client,
      close,
      emit: (event: ApiEvent) => listener?.(event),
      open: () => opened?.(),
    };
  }

  it("delivers a checked job.update and ignores other topics", () => {
    const { client, emit } = channel();
    const seen: unknown[] = [];
    new ClientRebuildJobSource(client).watch((event) => seen.push(event));
    emit({ topic: "health.change", data: {} });
    emit({ topic: "job.update", data: REBUILD_JOB.job });
    expect(seen).toEqual([{ kind: "update", job: REBUILD_JOB.job }]);
  });

  it("asks for a reread on events.resync and when the channel opens", () => {
    const { client, emit, open } = channel();
    const seen: string[] = [];
    new ClientRebuildJobSource(client).watch((event) => seen.push(event.kind));
    emit({ topic: "events.resync", data: { missed: 3 } });
    open();
    expect(seen).toEqual(["resync", "resync"]);
  });

  it("drops an update that breaks the contract and logs it", () => {
    const { client, emit } = channel();
    const warn = vi.spyOn(console, "warn").mockImplementation(() => undefined);
    const seen: string[] = [];
    new ClientRebuildJobSource(client).watch((event) => seen.push(event.kind));
    emit({ topic: "job.update", data: { id: "job-1" } });
    expect(seen).toEqual([]);
    expect(warn).toHaveBeenCalled();
    warn.mockRestore();
  });

  it("drops an update that carries a report, which an event never does", () => {
    const { client, emit } = channel();
    const warn = vi.spyOn(console, "warn").mockImplementation(() => undefined);
    const seen: string[] = [];
    new ClientRebuildJobSource(client).watch((event) => seen.push(event.kind));
    const result = { objects: [], unreadable: [] };
    emit({ topic: "job.update", data: { ...REBUILD_JOB.job, state: "completed", result } });
    expect(seen).toEqual([]);
    expect(warn).toHaveBeenCalled();
    warn.mockRestore();
  });

  it("closes the channel when the watcher is stopped", () => {
    const { client, close } = channel();
    const stop = new ClientRebuildJobSource(client).watch(() => undefined);
    stop();
    expect(close).toHaveBeenCalledOnce();
  });
});
