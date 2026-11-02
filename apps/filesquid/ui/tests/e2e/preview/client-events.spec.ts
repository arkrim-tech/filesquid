/**
 * Client-event delivery against the production build (`vite build` plus `vite preview`) (Task
 * UI-F2.16; UI-S-023, API-S-030; UI-S-013): the page's console warnings and errors reach `POST
 * /api/v1/client-events` as events of a level, one component, an error code and a client event ID,
 * never message text; after a 429 (with `Retry-After`), a 401 or 403, a 5xx and a network failure
 * the same events are sent again, every delivered event its own entry; a 400 batch is split into
 * single events and a refused one replaced by `client_event_refused`; a full queue spills into the
 * page's IndexedDB store; undelivered events are persisted at page close and delivered on the next
 * open; and with IndexedDB unavailable or refusing a write the page-close overflow count is sent
 * as a keep-alive request. The route is scripted per test; time is the browser's own (the waits
 * are the contract's 1 s backoff and a 2 s `Retry-After`).
 *
 * The page raises a few warnings of its own while it starts (the preview has no host for some
 * reads), and they are forwarded too. Each test therefore raises "probe" warnings, whose error
 * code is `test_probe`, and scripts and asserts only the requests that carry a probe; every other
 * request is acknowledged.
 */
import { expect, test, type Page, type Route } from "@playwright/test";
import { trackErrors } from "../support/strict-dom";
import { serveLogs } from "../support/logs-api";

/** One event as the page sends it. */
interface SentEvent {
  id: string;
  kind: string;
  level?: string;
  component: string;
  error_code?: string;
  count?: number;
}

/** One request the route saw. */
interface Seen {
  at: number;
  events: SentEvent[];
}

/** What the scripted route does with a request that carries a probe. */
type Answer = "ok" | "abort" | { status: number; headers?: Record<string, string> };

/** The error code of a probe warning. */
const PROBE = "test_probe";

/** True for an event a test raised. */
const isProbe = (event: SentEvent) => event.error_code === PROBE;

/** Serve the shell and a client-events route that answers probe requests as `script` says. */
async function serve(
  page: Page,
  script: (seen: Seen, index: number) => Answer,
): Promise<{ seen: Seen[]; journaled: Set<string>; all: Seen[] }> {
  await serveLogs(page);
  const seen: Seen[] = [];
  const all: Seen[] = [];
  const journaled = new Set<string>();
  await page.route(/\/api\/v1\/client-events$/, async (route: Route) => {
    const body = route.request().postDataJSON() as { events: SentEvent[] };
    const entry: Seen = { at: Date.now(), events: body.events };
    all.push(entry);
    const probing = body.events.some(isProbe);
    if (probing) seen.push(entry);
    const answer: Answer = probing ? script(entry, seen.length - 1) : "ok";
    if (answer === "abort") return route.abort("connectionreset");
    if (answer !== "ok") {
      return route.fulfill({
        status: answer.status,
        headers: answer.headers,
        json: {
          code: answer.status === 429 ? "rate_limited" : "internal",
          message: "x",
          retryable: true,
        },
      });
    }
    body.events.filter(isProbe).forEach((event) => journaled.add(event.id));
    return route.fulfill({
      json: {
        acknowledged: body.events.map((event) => event.id),
        journaled: body.events.length,
        principal: "urn:uuid:5a0c41e2-0000-4000-8000-00000000e19b",
      },
    });
  });
  return { seen, journaled, all };
}

/** Tolerate the browser's own line for a deliberately refused client-events request. */
const refused = (text: string, url: string) =>
  /\/api\/v1\/client-events$/.test(url) && /status of (40\d|429|50\d)|ERR_/.test(text);

/** Open the app and wait until the page's console is captured. */
async function ready(page: Page): Promise<void> {
  await page.goto("/#/settings");
  await page.waitForFunction(() => document.documentElement.dataset["clientEvents"] === "started");
}

/** Raise `count` probe warnings in one turn; the message text is private and never sent. */
function warn(page: Page, count: number): Promise<void> {
  return page.evaluate((n) => {
    for (let i = 0; i < n; i += 1) {
      const error = new Error("private message text IMG_4102.jpg");
      error.name = "TestProbe";
      console.warn("private message text IMG_4102.jpg", error);
    }
  }, count);
}

/** The ids of the probe events of every request, in order of arrival. */
const probeIds = (seen: Seen[]) => seen.flatMap((s) => s.events.filter(isProbe).map((e) => e.id));

test.describe("delivery", () => {
  test("sends a console warning as one event with a level, the fixed component, a code and an ID, no text", async ({
    page,
  }) => {
    const errors = trackErrors(page);
    const { seen } = await serve(page, () => "ok");
    await ready(page);
    await warn(page, 1);
    await expect.poll(() => seen.length).toBeGreaterThan(0);
    const event = seen[0]?.events.find(isProbe);
    expect(event).toMatchObject({
      kind: "console",
      level: "warn",
      component: "console",
      error_code: PROBE,
    });
    expect(event?.id).toMatch(
      /^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/,
    );
    expect(JSON.stringify(seen)).not.toContain("IMG_4102");
    expect(JSON.stringify(seen)).not.toContain("private message");
    expect(errors).toEqual([]);
  });

  test("retries after a 429 no sooner than Retry-After, with the same events, each journaled once", async ({
    page,
  }) => {
    const errors = trackErrors(page, refused);
    const { seen, journaled } = await serve(page, (_s, i) =>
      i === 0 ? { status: 429, headers: { "Retry-After": "2" } } : "ok",
    );
    await ready(page);
    await warn(page, 3);
    await expect.poll(() => journaled.size, { timeout: 20_000 }).toBe(3);
    expect(seen.length).toBe(2);
    expect((seen[1]?.at ?? 0) - (seen[0]?.at ?? 0)).toBeGreaterThanOrEqual(1900);
    expect(seen[1]?.events.map((e) => e.id)).toEqual(seen[0]?.events.map((e) => e.id));
    expect(new Set(probeIds(seen)).size).toBe(3);
    expect(errors).toEqual([]);
  });

  for (const [name, first] of [
    ["a 401", { status: 401 }],
    ["a 403", { status: 403 }],
    ["a 503", { status: 503 }],
    ["a network failure", "abort"],
  ] as const) {
    test(`retries after ${name} with backoff from 1 s, never merging events`, async ({ page }) => {
      const errors = trackErrors(page, refused);
      const { seen, journaled } = await serve(page, (_s, i) => (i === 0 ? first : "ok"));
      await ready(page);
      await warn(page, 3);
      await expect.poll(() => journaled.size, { timeout: 20_000 }).toBe(3);
      expect(seen.length).toBe(2);
      expect((seen[1]?.at ?? 0) - (seen[0]?.at ?? 0)).toBeGreaterThanOrEqual(900);
      expect(seen[1]?.events.map((e) => e.id)).toEqual(seen[0]?.events.map((e) => e.id));
      expect(errors).toEqual([]);
    });
  }

  test("splits a refused batch into single events, each delivered once as its own entry", async ({
    page,
  }) => {
    const errors = trackErrors(page, refused);
    const { seen, journaled, all } = await serve(page, (s) =>
      s.events.length > 1 ? { status: 400 } : "ok",
    );
    await ready(page);
    await warn(page, 3);
    await expect.poll(() => journaled.size, { timeout: 20_000 }).toBe(3);
    expect(seen[0]?.events.length).toBeGreaterThan(1);
    expect(seen.slice(1).filter((s) => s.events.length === 1).length).toBeGreaterThanOrEqual(3);
    expect(
      all.flatMap((s) => s.events).filter((e) => e.error_code === "client_event_refused"),
    ).toHaveLength(0);
    expect(errors).toEqual([]);
  });

  test("replaces an event the route refuses on its own with one client_event_refused event", async ({
    page,
  }) => {
    const errors = trackErrors(page, refused);
    const { journaled, all } = await serve(page, () => ({ status: 400 }));
    await ready(page);
    await warn(page, 2);
    await expect
      .poll(
        () =>
          all.flatMap((s) => s.events).filter((e) => e.error_code === "client_event_refused")
            .length,
        { timeout: 20_000 },
      )
      .toBe(2);
    const replacement = all
      .flatMap((s) => s.events)
      .find((e) => e.error_code === "client_event_refused");
    expect(replacement).toMatchObject({ kind: "console", level: "error", component: "console" });
    expect(journaled.size).toBe(0);
    expect(errors).toEqual([]);
  });
});

/** The number of probe events in the page's IndexedDB store. */
function stored(page: Page): Promise<number> {
  return page.evaluate(
    (code) =>
      new Promise<number>((resolve) => {
        const open = indexedDB.open("filesquid-client-events");
        open.onerror = () => resolve(-1);
        open.onsuccess = () => {
          const db = open.result;
          let found = 0;
          const cursor = db.transaction("events").objectStore("events").openCursor();
          cursor.onsuccess = () => {
            const at = cursor.result;
            if (!at) {
              db.close();
              resolve(found);
              return;
            }
            if ((at.value as { error_code?: string }).error_code === code) found += 1;
            at.continue();
          };
          cursor.onerror = () => resolve(-1);
        };
      }),
    PROBE,
  );
}

test.describe("storage", () => {
  test("writes each queued event through to IndexedDB as it is queued, before any page close", async ({
    page,
  }) => {
    const errors = trackErrors(page, refused);
    await serve(page, () => ({ status: 503 }));
    await ready(page);
    await warn(page, 3);
    await expect.poll(() => stored(page), { timeout: 10_000 }).toBe(3);
    expect(errors).toEqual([]);
  });

  test("a full queue spills into IndexedDB and loses nothing", async ({ page }) => {
    test.setTimeout(240_000);
    const errors = trackErrors(page, refused);
    await serve(page, () => ({ status: 503 }));
    await ready(page);
    await warn(page, 10_005);
    await expect.poll(() => stored(page), { timeout: 200_000, intervals: [2000] }).toBe(10_005);
    expect(errors).toEqual([]);
  });

  test("events persisted at page close are delivered, oldest first, on the next open", async ({
    page,
  }) => {
    const errors = trackErrors(page, refused);
    let healthy = false;
    const { seen } = await serve(page, () => (healthy ? "ok" : { status: 503 }));
    await ready(page);
    await warn(page, 3);
    await expect.poll(() => stored(page)).toBe(3);
    await expect.poll(() => probeIds(seen).length).toBeGreaterThan(0);
    const original = [...new Set(probeIds(seen))];
    expect(original).toHaveLength(3);
    healthy = true;
    const before = seen.length;
    await page.reload();
    await page.waitForFunction(
      () => document.documentElement.dataset["clientEvents"] === "started",
    );
    await expect
      .poll(() => new Set(probeIds(seen.slice(before))).size, { timeout: 20_000 })
      .toBe(3);
    expect([...new Set(probeIds(seen.slice(before)))]).toEqual(original);
    await expect.poll(() => stored(page)).toBe(0);
    expect(errors).toEqual([]);
  });
});

/** Record `keepalive` on each client-events fetch (`window.__fsKeepalive`) before the page loads. */
async function recordKeepalive(page: Page): Promise<void> {
  await page.addInitScript(() => {
    const win = window as unknown as { __fsKeepalive: boolean[] };
    win.__fsKeepalive = [];
    const original = window.fetch.bind(window);
    window.fetch = (input: RequestInfo | URL, init?: RequestInit) => {
      const url = typeof input === "string" ? input : input instanceof URL ? input.href : input.url;
      if (url.includes("/api/v1/client-events")) win.__fsKeepalive.push(init?.keepalive === true);
      return original(input, init);
    };
  });
}

/** The keep-alive flags recorded for client-events fetches in this page. */
function keepaliveFlags(page: Page): Promise<boolean[]> {
  return page.evaluate(() => {
    const win = window as unknown as { __fsKeepalive?: boolean[] };
    return win.__fsKeepalive ?? [];
  });
}

test.describe("the one gap", () => {
  test("counts the events it could not keep when IndexedDB is unavailable, and sends it at page close", async ({
    page,
  }) => {
    const errors = trackErrors(page, refused);
    await page.addInitScript(() => {
      Object.defineProperty(window, "indexedDB", { value: undefined, configurable: true });
    });
    await recordKeepalive(page);
    const { all } = await serve(page, () => ({ status: 503 }));
    await ready(page);
    await warn(page, 4);
    await page.evaluate(() => window.dispatchEvent(new Event("pagehide")));
    await expect
      .poll(() => all.some((s) => s.events.some((e) => e.kind === "overflow")), { timeout: 10_000 })
      .toBe(true);
    const overflow = all.flatMap((s) => s.events).find((e) => e.kind === "overflow");
    // The four probes and any warning the page raised itself, none of which could be kept.
    expect(overflow?.count).toBeGreaterThanOrEqual(4);
    expect(overflow).toMatchObject({ kind: "overflow", component: "client-events" });
    expect(overflow?.id).toMatch(/^[0-9a-f-]{36}$/);
    expect(overflow).not.toHaveProperty("error_code");
    expect(await keepaliveFlags(page)).toContain(true);
    expect(errors).toEqual([]);
  });

  test("counts them too when IndexedDB refuses a write", async ({ page }) => {
    const errors = trackErrors(page, refused);
    await page.addInitScript(() => {
      IDBObjectStore.prototype.put = function refuse() {
        throw new DOMException("The quota has been exceeded", "QuotaExceededError");
      };
    });
    await recordKeepalive(page);
    const { all } = await serve(page, () => ({ status: 503 }));
    await ready(page);
    await warn(page, 4);
    await page.evaluate(() => window.dispatchEvent(new Event("pagehide")));
    await expect
      .poll(() => all.some((s) => s.events.some((e) => e.kind === "overflow")), { timeout: 10_000 })
      .toBe(true);
    const overflow = all.flatMap((s) => s.events).find((e) => e.kind === "overflow");
    expect(overflow?.count).toBeGreaterThanOrEqual(4);
    expect(overflow).toMatchObject({ kind: "overflow", component: "client-events" });
    expect(overflow?.id).toMatch(/^[0-9a-f-]{36}$/);
    expect(overflow).not.toHaveProperty("error_code");
    expect(await keepaliveFlags(page)).toContain(true);
    expect(errors).toEqual([]);
  });
});
