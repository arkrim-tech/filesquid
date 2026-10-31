/**
 * Unit tests for the upload flow's contract checks and sources (UI-1.2, UI-S-012, API-S-036,
 * API-S-037): every call goes through the client to the contract's route with the contract's
 * body, every answer is checked before it is returned (a host that answers outside the contract
 * is a `TypeError`), API refusals pass through unchanged, and `job.update` events become job
 * changes with one shared subscription.
 */
import { describe, expect, it, vi } from "vitest";
import {
  parseImportAnswer,
  parseJobControlAnswer,
  parseJobItemPage,
  parseJobPage,
  parsePlanItemPage,
} from "../../src/imports/contract";
import { ClientImportSource } from "../../src/imports/client-source";
import { ClientJobSource } from "../../src/imports/jobs-source";
import {
  clientDestinationLookup,
  parseResolvedDestination,
} from "../../src/imports/destination-source";
import {
  COLLECTION_ID,
  FakeClient,
  LIBRARY_ID,
  MOCKUP_ITEMS,
  PLAN,
  PLAN_ITEMS,
  jobView,
  refusal,
} from "../support/import-fakes";

const outcome = (kind: "pending" | "committed", id = "op-1") => ({
  operation_id: id,
  kind,
  error: null,
});

describe("answers are checked against the contract (API-S-036, API-S-037)", () => {
  it("reads a planned, a started and a cancelled import", () => {
    expect(
      parseImportAnswer({
        outcome: outcome("pending"),
        import: "planned",
        plan_id: "plan-1",
        plan: PLAN,
      }),
    ).toEqual({ kind: "planned", plan: PLAN });
    expect(
      parseImportAnswer({ outcome: outcome("pending"), import: "started", job_id: "job-1" }),
    ).toEqual({ kind: "started", jobId: "job-1" });
    expect(parseImportAnswer({ outcome: outcome("committed"), import: "cancelled" })).toEqual({
      kind: "cancelled",
    });
  });

  it("refuses an import answer that is not one of the three, or lacks what it names", () => {
    expect(() => parseImportAnswer(null)).toThrow(TypeError);
    expect(() => parseImportAnswer({ outcome: outcome("pending"), import: "planned" })).toThrow(
      TypeError,
    );
    expect(() => parseImportAnswer({ outcome: outcome("pending"), import: "started" })).toThrow(
      TypeError,
    );
    expect(() => parseImportAnswer({ outcome: outcome("pending"), import: "bogus" })).toThrow(
      TypeError,
    );
    expect(() =>
      parseImportAnswer({
        outcome: outcome("pending"),
        import: "planned",
        plan: { ...PLAN, files_new: -1 },
      }),
    ).toThrow(TypeError);
  });

  it("keeps a plan's optional facts optional and its free space nullable", () => {
    const bare = { ...PLAN, source_volume: undefined, free_bytes_after: null };
    const parsed = parseImportAnswer({
      outcome: outcome("pending"),
      import: "planned",
      plan_id: "plan-1",
      plan: bare,
    });
    expect(parsed.kind === "planned" && parsed.plan.free_bytes_after).toBeNull();
    expect(parsed.kind === "planned" && parsed.plan.source_volume).toBeUndefined();
  });

  it("reads a page of planned files and a page of a job's files", () => {
    expect(parsePlanItemPage({ items: PLAN_ITEMS, next_cursor: null })).toEqual({
      items: PLAN_ITEMS,
      next_cursor: null,
    });
    expect(parseJobItemPage({ items: MOCKUP_ITEMS, next_cursor: "c2" }).next_cursor).toBe("c2");
    expect(() => parsePlanItemPage({ items: [{ item_id: "p" }], next_cursor: null })).toThrow(
      TypeError,
    );
    expect(() =>
      parseJobItemPage({ items: [{ ...MOCKUP_ITEMS[0], state: "bogus" }], next_cursor: null }),
    ).toThrow(TypeError);
  });

  it("reads the job list and a control's answer, and drops fields outside the contract", () => {
    const view = { ...jobView(), checkpoint: "secret", lease_holder: "w1" };
    const page = parseJobPage({ jobs: [view], truncated: false });
    expect(page.jobs[0]).toEqual(jobView());
    expect(page.truncated).toBe(false);
    expect(parseJobControlAnswer({ job: view, scheduled: true })).toEqual(jobView());
    expect(() => parseJobControlAnswer({ job: { id: "j" }, scheduled: true })).toThrow(TypeError);
    expect(() => parseJobPage({ jobs: "no", truncated: false })).toThrow(TypeError);
  });
});

describe("ClientImportSource (API-S-036)", () => {
  /** A client whose import route answers `answer`. */
  function setup(answer: unknown) {
    const client = new FakeClient({
      "POST /api/v1/imports": answer,
      "POST /api/v1/imports/plan-1/start": {
        outcome: outcome("pending"),
        import: "started",
        job_id: "job-1",
      },
      "DELETE /api/v1/imports/plan-1": { plan_id: "plan-1", discarded: true },
      "GET /api/v1/imports/plan-1/items": { items: PLAN_ITEMS, next_cursor: "c2" },
      "GET /api/v1/imports/plan-1/items?cursor=c2&limit=500": {
        items: [],
        next_cursor: null,
      },
    });
    return { client, source: new ClientImportSource(client) };
  }

  it("plans an import with the envelope, the library, the destination and the source", async () => {
    const { client, source } = setup({
      outcome: outcome("pending"),
      import: "planned",
      plan_id: "plan-1",
      plan: PLAN,
    });
    const answer = await source.plan({
      libraryId: LIBRARY_ID,
      collectionId: COLLECTION_ID,
      folder: "Site photos",
      source: "files",
      operationId: "op-7",
    });
    expect(answer).toEqual({ kind: "planned", plan: PLAN });
    expect(client.seen[0]).toEqual({
      path: "/api/v1/imports",
      method: "POST",
      body: {
        envelope: {
          schema_version: 1,
          operation_id: "op-7",
          expected_revision: "0",
          scope: `library:${LIBRARY_ID}`,
        },
        library_id: LIBRARY_ID,
        collection_id: COLLECTION_ID,
        folder: "Site photos",
        source: "files",
      },
    });
  });

  it("sends no folder for a collection's root and never a path", async () => {
    const { client, source } = setup({ outcome: outcome("committed"), import: "cancelled" });
    const answer = await source.plan({
      libraryId: LIBRARY_ID,
      collectionId: COLLECTION_ID,
      source: "folder",
    });
    expect(answer).toEqual({ kind: "cancelled" });
    const body = client.seen[0]?.body as Record<string, unknown>;
    expect("folder" in body).toBe(false);
    expect(body.source).toBe("folder");
    expect(JSON.stringify(body)).not.toMatch(/\/Users|\\\\|file:/);
    expect((body.envelope as { operation_id: string }).operation_id).toMatch(/^[0-9a-f-]{36}$/);
  });

  it("lists planned files, starts the job and discards the plan on their routes", async () => {
    const { client, source } = setup({ outcome: outcome("committed"), import: "cancelled" });
    expect((await source.planItems("plan-1")).next_cursor).toBe("c2");
    await source.planItems("plan-1", "c2", 500);
    expect(await source.start("plan-1")).toBe("job-1");
    await source.discard("plan-1");
    expect(client.seen.map((seen) => `${seen.method} ${seen.path}`)).toEqual([
      "GET /api/v1/imports/plan-1/items",
      "GET /api/v1/imports/plan-1/items?cursor=c2&limit=500",
      "POST /api/v1/imports/plan-1/start",
      "DELETE /api/v1/imports/plan-1",
    ]);
    expect(client.seen[2]?.body).toBeUndefined();
  });

  it("refuses a start that answers anything but a started job", async () => {
    const client = new FakeClient({
      "POST /api/v1/imports/plan-1/start": { outcome: outcome("committed"), import: "cancelled" },
    });
    await expect(new ClientImportSource(client).start("plan-1")).rejects.toThrow(TypeError);
  });

  it("passes the API's refusals through unchanged", async () => {
    const error = refusal(404, "not_found");
    const client = new FakeClient({ "POST /api/v1/imports": error });
    const source = new ClientImportSource(client);
    await expect(
      source.plan({ libraryId: LIBRARY_ID, collectionId: "gone", source: "files" }),
    ).rejects.toBe(error);
  });

  it("encodes a plan ID in the path", async () => {
    const client = new FakeClient({
      "DELETE /api/v1/imports/a%2Fb": { plan_id: "a/b", discarded: true },
    });
    await new ClientImportSource(client).discard("a/b");
    expect(client.seen[0]?.path).toBe("/api/v1/imports/a%2Fb");
  });
});

describe("ClientJobSource (API-S-037)", () => {
  /** A client with every job route. */
  function setup() {
    const client = new FakeClient({
      "GET /api/v1/jobs?kind=import&limit=100": { jobs: [jobView()], truncated: false },
      "GET /api/v1/jobs/job-1/items?limit=200": { items: MOCKUP_ITEMS, next_cursor: null },
      "POST /api/v1/jobs/job-1/pause": { job: jobView({ paused: true }), scheduled: false },
      "POST /api/v1/jobs/job-1/resume": { job: jobView(), scheduled: true },
      "POST /api/v1/jobs/job-1/cancel": { job: jobView({ control: "cancel" }), scheduled: false },
      "POST /api/v1/jobs/job-1/items/item-IMG_4099.JPG/retry": {
        job: jobView(),
        scheduled: true,
      },
      "POST /api/v1/jobs/job-1/retry-failed": { job: jobView(), scheduled: true },
    });
    return { client, source: new ClientJobSource(client) };
  }

  it("lists the import jobs, reads a job's files and sends each control with no body", async () => {
    const { client, source } = setup();
    expect((await source.list()).jobs).toEqual([jobView()]);
    expect((await source.items("job-1", null, 200)).items).toEqual(MOCKUP_ITEMS);
    expect((await source.pause("job-1")).paused).toBe(true);
    await source.resume("job-1");
    expect((await source.cancel("job-1")).control).toBe("cancel");
    await source.retryItem("job-1", "item-IMG_4099.JPG");
    await source.retryFailed("job-1");
    const posts = client.seen.filter((seen) => seen.method === "POST");
    expect(posts.map((seen) => seen.path)).toEqual([
      "/api/v1/jobs/job-1/pause",
      "/api/v1/jobs/job-1/resume",
      "/api/v1/jobs/job-1/cancel",
      "/api/v1/jobs/job-1/items/item-IMG_4099.JPG/retry",
      "/api/v1/jobs/job-1/retry-failed",
    ]);
    expect(posts.every((seen) => seen.body === undefined)).toBe(true);
  });

  it("shares one event subscription and turns job.update into a job change", () => {
    const { client, source } = setup();
    const heard: unknown[] = [];
    const stopA = source.subscribe((change) => heard.push(change));
    const stopB = source.subscribe((change) => heard.push(change));
    expect(client.subscriptions).toBe(1);
    client.emit({ topic: "job.update", data: jobView({ done: 90 }) });
    expect(heard).toEqual([
      { kind: "job", job: jobView({ done: 90 }) },
      { kind: "job", job: jobView({ done: 90 }) },
    ]);
    stopA();
    expect(client.listening).toBe(1);
    stopB();
    expect(client.listening).toBe(0);
  });

  it("asks to read the jobs again after a resync, a reopened channel or a broken payload", () => {
    const { client, source } = setup();
    const heard: unknown[] = [];
    const warn = vi.spyOn(console, "warn").mockImplementation(() => undefined);
    source.subscribe((change) => heard.push(change));
    client.emit({ topic: "events.resync", data: { missed: 3 } });
    client.open();
    client.emit({ topic: "job.update", data: { id: "j" } });
    client.emit({ topic: "navigation.change", data: { all: true } });
    expect(heard).toEqual([{ kind: "resync" }, { kind: "resync" }, { kind: "resync" }]);
    expect(warn).toHaveBeenCalled();
    warn.mockRestore();
  });
});

describe("clientDestinationLookup (API-S-021, API-S-036)", () => {
  it("asks the destination route for a node and checks the answer", async () => {
    const client = new FakeClient({
      "GET /api/v1/navigation/destination?node=c%2F1": {
        library_id: LIBRARY_ID,
        collection_id: COLLECTION_ID,
        path: "/Users/x/secret",
      },
    });
    const found = await clientDestinationLookup(client)("c/1");
    expect(found).toEqual({ libraryId: LIBRARY_ID, collectionId: COLLECTION_ID });
  });

  it("answers nothing, and logs, when the host has no such route or the answer is incomplete", async () => {
    const warn = vi.spyOn(console, "warn").mockImplementation(() => undefined);
    const gone = new FakeClient({
      "GET /api/v1/navigation/destination?node=c1": refusal(404, "not_found"),
    });
    expect(await clientDestinationLookup(gone)("c1")).toBeUndefined();
    expect(warn).toHaveBeenCalledTimes(1);
    expect(parseResolvedDestination({ library_id: LIBRARY_ID })).toBeUndefined();
    expect(parseResolvedDestination(null)).toBeUndefined();
    warn.mockRestore();
  });
});
