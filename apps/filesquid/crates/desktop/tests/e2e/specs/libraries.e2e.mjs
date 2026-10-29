/**
 * Packaged-window proof of the library routes (Task DE-1.1; API-S-025, SRS-F-001, DE-S-006):
 * in a real Tauri webview, the app page creates a library in an owner-chosen folder, opens
 * the media library's folder, copies its completed snapshot into an empty folder, rebuilds
 * it from its files, lists its repair candidates, sees a repair without the owner's consent
 * refused, and starts the history backfill, all through `filesquid://` with its per-launch
 * token. Folders are named by opaque location IDs only, and no answer carries a path.
 *
 * Each `browser.execute` body runs in the app page, so it cannot use this module's scope; the
 * values it needs are passed in as arguments.
 */
import { browser, expect } from "@wdio/globals";
import { existsSync } from "node:fs";
import path from "node:path";
import { E2E_LIBRARY_ID, E2E_OBJECT_ID } from "../support/media-library.mjs";
import {
  COPY_LIBRARY_ID,
  COPY_LOCATION,
  CREATE_LOCATION,
  MEDIA_LOCATION,
  removeHeadInventories,
} from "../support/library-folders.mjs";

/**
 * The scheme base URL the app page uses on this platform (ADR 030 Decision 3).
 *
 * @returns `http://filesquid.localhost` on Windows, `filesquid://localhost` elsewhere.
 */
function apiBase() {
  return process.platform === "win32" ? "http://filesquid.localhost" : "filesquid://localhost";
}

/**
 * Call the API from the app page with its per-launch token.
 *
 * @param method The HTTP method.
 * @param route The path below the scheme base.
 * @param body The JSON body, or `null` for none.
 * @returns The status, the answer text and its JSON (or `null`).
 */
async function call(method, route, body) {
  return browser.execute(
    async (base, verb, target, payload) => {
      const token = window.__FILESQUID_LAUNCH_TOKEN__;
      const headers = { Authorization: `Bearer ${token}` };
      if (payload !== null) {
        headers["Content-Type"] = "application/json";
      }
      const response = await fetch(`${base}${target}`, {
        method: verb,
        headers,
        body: payload === null ? undefined : JSON.stringify(payload),
      });
      const text = await response.text();
      let json = null;
      try {
        json = JSON.parse(text);
      } catch {
        json = null;
      }
      return { status: response.status, text, json };
    },
    apiBase(),
    method,
    route,
    body,
  );
}

/** Most reads of a job by {@link finishedJob}, 50 ms apart. */
const MAX_JOB_POLLS = 200;

/**
 * Read a job through the job route until it has finished (bounded by MAX_JOB_POLLS).
 *
 * @param id The job ID.
 * @returns The last answer of `GET /api/v1/jobs/{job}`.
 */
async function finishedJob(id) {
  let answer = null;
  for (let poll = 0; poll < MAX_JOB_POLLS; poll += 1) {
    answer = await call("GET", `/api/v1/jobs/${id}`, null);
    if (["completed", "failed", "cancelled"].includes(answer.json?.state)) return answer;
    await browser.pause(50);
  }
  throw new Error(`the job did not finish: ${JSON.stringify(answer?.json)}`);
}

/**
 * Assert an answer names no folder of this run: neither the run directory nor a location's
 * folder name.
 *
 * @param answer The answer of {@link call}.
 */
function expectNoPath(answer) {
  const run = process.env.FILESQUID_DESKTOP_E2E_RUN ?? "";
  if (run !== "") {
    expect(answer.text).not.toContain(run);
  }
  expect(answer.text).not.toContain("/locations/");
}

describe("the library routes in a packaged window", () => {
  before(async () => {
    await browser.waitUntil(
      async () => (await browser.execute(() => document.readyState)) === "complete",
      { timeout: 60000, timeoutMsg: "the app page did not load" },
    );
  });

  it("creates a library in an owner-chosen folder and answers an opaque ID", async () => {
    const created = await call("POST", "/api/v1/libraries", { location_id: CREATE_LOCATION });
    expect(created.status).toBe(200);
    expect(created.json.spec_version).toBe("1.1");
    expect(created.json.objects).toBe(0);
    expect(created.json.library_id).toMatch(/^[0-9a-f-]{36}$/);
    expectNoPath(created);
  });

  it("opens, copies and rebuilds the media library through its location ID", async () => {
    const opened = await call("POST", "/api/v1/libraries/openings", {
      location_id: MEDIA_LOCATION,
      library_id: E2E_LIBRARY_ID,
    });
    expect(opened.status).toBe(200);
    expect(opened.json.library_id).toBe(E2E_LIBRARY_ID);
    expect(opened.json.objects).toBeGreaterThan(0);
    const copied = await call("POST", "/api/v1/libraries/snapshot-copies", {
      library_id: E2E_LIBRARY_ID,
      destination_location_id: COPY_LOCATION,
    });
    expect(copied.status).toBe(200);
    expect(copied.json.invalid_objects).toEqual([]);
    const rebuilt = await call("POST", "/api/v1/libraries/rebuilds", {
      library_id: E2E_LIBRARY_ID,
    });
    expect(rebuilt.status).toBe(200);
    expect(rebuilt.json.outcome).toBe("pending");
    expect(rebuilt.json.job.kind).toBe("library.rebuild");
    const job = await finishedJob(rebuilt.json.job_id);
    expect(job.json.state).toBe("completed");
    expect(job.json.result.objects.length).toBe(opened.json.objects);
    for (const answer of [opened, copied, rebuilt, job]) {
      expectNoPath(answer);
    }
  });

  it("finds the copy's object without a version inventory and repairs it only with consent", async () => {
    const copy = path.join(process.env.FILESQUID_E2E_LOCATIONS, "copy");
    const heads = removeHeadInventories(copy);
    expect(heads.length).toBe(1);
    const opened = await call("POST", "/api/v1/libraries/openings", {
      location_id: COPY_LOCATION,
      library_id: COPY_LIBRARY_ID,
    });
    expect(opened.status).toBe(200);
    const query = `/api/v1/libraries/repair-candidates?library_id=${COPY_LIBRARY_ID}`;
    const candidates = await call("GET", query, null);
    expect(candidates.status).toBe(200);
    expect(candidates.json.candidates.map((c) => c.object_id)).toEqual([E2E_OBJECT_ID]);
    const objects = [E2E_OBJECT_ID];
    const refused = await call("POST", "/api/v1/libraries/repairs", {
      library_id: COPY_LIBRARY_ID,
      object_ids: objects,
    });
    expect(refused.status).toBe(409);
    expect(existsSync(path.join(heads[0], "inventory.json"))).toBe(false);
    const repaired = await call("POST", "/api/v1/libraries/repairs", {
      library_id: COPY_LIBRARY_ID,
      object_ids: objects,
      consent: {
        library_id: COPY_LIBRARY_ID,
        object_ids: objects,
        statement_id: "version_inventory_repair.v2",
        confirmed: true,
      },
    });
    expect(repaired.status).toBe(200);
    expect(repaired.json.objects[0].result).toBe("repaired");
    expect(existsSync(path.join(heads[0], "inventory.json"))).toBe(true);
    for (const answer of [opened, candidates, refused, repaired]) {
      expectNoPath(answer);
    }
  });

  it("starts the history backfill over the served roots", async () => {
    const started = await call("POST", "/api/v1/admin/history/backfill", null);
    expect([200, 409]).toContain(started.status);
    if (started.status === 200) {
      expect(started.json.status).toBe("started");
    }
  });
});
