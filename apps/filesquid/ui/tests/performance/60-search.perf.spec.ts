/**
 * Search on the production build (Task UI-F3.1, NFR-002: indexed metadata search p95 <= 2 s;
 * PRD F-063: "Query feedback and completed results MUST be measured separately under NFR-002").
 * 200 varied queries after a warm-up of 20, typed into the real header search bar:
 *
 * - query feedback: the field shows the query and the drop-down is open (p95 <= 100 ms);
 * - live files: the drop-down's first file is the host's top hit (the bar waits 250 ms before
 *   it asks, by design); completed within p95 <= 2,000 ms;
 * - completed results: Enter opens the results and the first rows are the host's relevance
 *   order, the first page painted, p95 <= 2,000 ms.
 *
 * The host searches the 10,000-file view of the Large catalog by name, in a stable relevance
 * order (document order), paged by 200. The host's own indexing time is not in these numbers;
 * the packaged host's is measured by DE-F3.2.
 */
import { expect, test } from "@playwright/test";
import { BUDGETS } from "./budgets";
import { Catalogue, PROFILES } from "./fixtures";
import { RESULTS, VIEW_LOCATOR, idle, openView } from "./harness";
import type { Pred } from "./probe";
import { assertBudgets, latency, publish } from "./report";

const BAR = "fs-app-shell >> fs-search-bar";
const FIELD = `${BAR} >> input.text`;

test("search: query feedback, live files and completed results over 200 queries", async ({
  page,
}, info) => {
  const catalogue = new Catalogue(PROFILES.large);
  const session = await openView(page, catalogue);
  const { probe } = session;
  const total = BUDGETS.samples.warmup + BUDGETS.samples.warm;
  const terms = catalogue.searchTerms(total);
  expect(terms.length).toBe(total);
  const feedback: number[] = [];
  const live: number[] = [];
  const completed: number[] = [];
  const field = page.locator("fs-search-bar input.text");
  for (let i = 0; i < total; i += 1) {
    const q = terms[i] as string;
    const found = catalogue.searchPage({ q, sort: "relevance" }, null, 200).rows;
    const names = found.map((row) => (row.kind === "file" ? row.display_name : row.name));
    await field.click();
    await field.fill("");
    await idle(page);
    const shown: Pred = {
      k: "all",
      of: [
        { k: "value", at: FIELD, equals: q },
        { k: "attr", at: FIELD, name: "aria-expanded", value: "true" },
      ],
    };
    const liveFiles: Pred = {
      k: "all",
      of: [shown, { k: "text", at: `${BAR} >> .file-option`, includes: names[0] as string }],
    };
    const typing = await probe.measure(
      { events: ["input"], ack: shown, ready: liveFiles, timeoutMs: BUDGETS.sampleTimeoutMs },
      () => page.keyboard.insertText(q),
    );
    const rows: Pred = {
      k: "all",
      of: [
        { k: "attr", at: RESULTS, name: "data-state", value: "ready" },
        { k: "count", at: `${RESULTS} >> .row`, min: Math.min(10, found.length) },
        { k: "absent", at: `${RESULTS} >> .placeholder-row` },
        ...names.slice(0, 3).map((equals, index) => ({
          k: "text" as const,
          at: `${RESULTS} >> .row[data-index='${index}'] >> .name`,
          equals,
        })),
      ],
    };
    const results = await probe.measure(
      { events: ["keydown"], ready: rows, timeoutMs: BUDGETS.sampleTimeoutMs },
      () => page.keyboard.press("Enter"),
    );
    if (
      typing.timedOut ||
      results.timedOut ||
      typing.readyMs === null ||
      results.readyMs === null
    ) {
      throw new Error(`query ${i} never completed within ${BUDGETS.sampleTimeoutMs} ms`);
    }
    if (i >= BUDGETS.samples.warmup) {
      feedback.push(typing.ackMs ?? typing.readyMs);
      live.push(typing.readyMs);
      completed.push(results.readyMs);
    }
    await page.evaluate(() => void (location.hash = "#/navigation/c-0"));
    await expect(page.locator(VIEW_LOCATOR)).toHaveAttribute("data-state", "ready");
    await idle(page);
  }
  const limit = {
    minSamples: BUDGETS.samples.minimum,
    p95Max: BUDGETS.warmFeedbackP95Ms,
    p99Max: BUDGETS.warmTailP99Ms,
  };
  const records = [
    latency({
      id: "search-query-feedback",
      title:
        "Search: query feedback (the field shows the query, the drop-down is open), Large catalog",
      budget: "NFR-002 acknowledgment p95 <= 100 ms; p99 <= 200 ms",
      samples: feedback,
      warmup: BUDGETS.samples.warmup,
      limit,
    }),
    latency({
      id: "search-live-files",
      title: "Search: the drop-down shows the host's top hit (includes the bar's 250 ms wait)",
      budget: "NFR-002 indexed metadata search p95 <= 2 s",
      samples: live,
      warmup: BUDGETS.samples.warmup,
      limit: { minSamples: BUDGETS.samples.minimum, p95Max: BUDGETS.searchCompletedP95Ms },
    }),
    latency({
      id: "search-completed-results",
      title: "Search: Enter to the first page of results painted in the host's order",
      budget: "NFR-002 indexed metadata search p95 <= 2 s",
      samples: completed,
      warmup: BUDGETS.samples.warmup,
      limit: { minSamples: BUDGETS.samples.minimum, p95Max: BUDGETS.searchCompletedP95Ms },
      notes: {
        distinctQueries: new Set(terms).size,
        searchCalls: session.host.calls.get("search") ?? 0,
      },
    }),
  ];
  await publish(info, records);
  assertBudgets(records);
  expect(session.errors).toEqual([]);
  expect(session.host.unserved).toEqual([]);
});
