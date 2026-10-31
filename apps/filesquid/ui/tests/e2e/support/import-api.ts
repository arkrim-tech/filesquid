/**
 * A fake host for the upload flow's production-build Playwright tests (UI-1.2, API-S-036,
 * API-S-037, API-S-021): the page is the real built UI, and these routes answer as the desktop
 * host does: the host descriptor (desktop, so the New menu's upload items are on), the navigation
 * tree of mockup 52, the destination route the platform engineer is to add, the import routes
 * (plan, planned files, start, discard) and the job routes (list, files, controls) with
 * `job.update` events pushed over the event channel. The import and job answers come from the
 * package's own memory sources, which answer as the API states, and every call is recorded so a
 * test can assert what the page sent and, above all, what it did not send.
 */
import type { Page, WebSocketRoute } from "@playwright/test";
import { ApiError } from "../../../src/api/client";
import {
  MemoryImportSource,
  MemoryJobSource,
  type MemoryImportOptions,
} from "../../../src/imports/memory-source";
import type { ImportPlan, JobItem, JobView, PlanItem } from "../../../src/imports/model";
import type { FixtureNode } from "../../../src/navigation/source";
import { PLAN, PLAN_ITEMS, MOCKUP_ITEMS, jobView } from "../../support/import-fakes";
import { ROWS, serveDrive } from "./drive-api";
import { serveNavigation } from "./navigation-api";

export { PLAN, PLAN_ITEMS, MOCKUP_ITEMS, jobView };

/** The library of the fixtures. */
export const LIBRARY = "lib-1";

/** The tree of mockup 52: two libraries, the first with collections and folders. */
export const TREE: FixtureNode = {
  node: "r",
  kind: "repository",
  name: null,
  children: [
    {
      node: "l1",
      kind: "library",
      name: "Family archive",
      children: [
        { node: "c1", kind: "collection", name: "Documents" },
        {
          node: "c2",
          kind: "collection",
          name: "Harbour project",
          children: [
            { node: "f1", kind: "folder", name: "Contracts" },
            { node: "f2", kind: "folder", name: "Drawings" },
            { node: "f3", kind: "folder", name: "Site photos" },
          ],
        },
        { node: "c3", kind: "collection", name: "Photos 2026" },
        { node: "v1", kind: "virtual_collection", name: "Best of 2026" },
      ],
    },
    {
      node: "l2",
      kind: "library",
      name: "Studio work",
      children: [{ node: "c9", kind: "collection", name: "Clients" }],
    },
  ],
};

/** The collection ID of each destination node of {@link TREE}. */
const COLLECTION_OF: Readonly<Record<string, string>> = {
  c1: "col-documents",
  c2: "col-harbour",
  f1: "col-harbour",
  f2: "col-harbour",
  f3: "col-harbour",
  c3: "col-photos",
  c9: "col-clients",
};

/** What the fake saw and how to steer it. */
export interface ImportApi {
  /** The import source behind the routes; its `requests` and `calls` record what was sent. */
  readonly imports: MemoryImportSource;
  /** The job source behind the routes; its `calls` record each control. */
  readonly jobs: MemoryJobSource;
  /** Every `POST /api/v1/imports` body as the page sent it, in order. */
  readonly planBodies: Record<string, unknown>[];
  /** Every destination lookup's node, in order. */
  readonly lookups: string[];
  /** Hold the plan's answer for this many milliseconds, to show the planning state. */
  planDelayMs: number;
  /** Push a `job.update` to the page, replacing or adding the job in the fake. */
  emit(job: JobView): void;
  /** Answer `POST /api/v1/imports` with this status and code instead of the plan. */
  refuseWith?: { status: number; code: string };
}

/** The options of {@link serveImports}. */
export interface ServeImportsOptions {
  /** What the import source answers. */
  readonly imports?: MemoryImportOptions;
  /** The jobs the host already has and their files. */
  readonly jobs?: readonly JobView[];
  readonly items?: Record<string, JobItem[]>;
  /** The nodes the drive view serves, so a collection or folder can be opened. */
  readonly nodes?: readonly string[];
}

/** The desktop host's descriptor. */
const DESKTOP = {
  schema_version: 1,
  host_type: "desktop",
  operator_name: null,
  hosted_service: {
    enrolled: false,
    backup_enabled: false,
    sync_enabled: false,
    service_host: null,
  },
};

/** An `ApiError` body as the API writes it. */
const errorBody = (code: string) => ({ code, message: "refused", retryable: false });

/** Answer a route from a function, turning an `ApiError` into its status and body. */
async function answer(
  route: import("@playwright/test").Route,
  produce: () => Promise<unknown> | unknown,
): Promise<void> {
  try {
    await route.fulfill({ json: (await produce()) as object });
  } catch (error) {
    const status = error instanceof ApiError ? error.status : 500;
    const code = error instanceof ApiError ? (error.code ?? "internal") : "internal";
    await route.fulfill({ status, json: errorBody(code) });
  }
}

/** Serve the import routes. */
async function serveImportRoutes(page: Page, api: ImportApi): Promise<void> {
  await page.route(/\/api\/v1\/imports$/, async (route) => {
    const body = route.request().postDataJSON() as {
      envelope: { operation_id: string };
      library_id: string;
      collection_id: string;
      folder?: string;
      source: "files" | "folder";
    };
    api.planBodies.push(body);
    if (api.planDelayMs > 0) await new Promise((resolve) => setTimeout(resolve, api.planDelayMs));
    if (api.refuseWith) {
      const { status, code } = api.refuseWith;
      await route.fulfill({ status, json: errorBody(code) });
      return;
    }
    const outcome = (kind: string) => ({
      operation_id: body.envelope.operation_id,
      kind,
      error: null,
    });
    await answer(route, async () => {
      const planned = await api.imports.plan({
        libraryId: body.library_id,
        collectionId: body.collection_id,
        folder: body.folder,
        source: body.source,
        operationId: body.envelope.operation_id,
      });
      if (planned.kind === "planned") {
        return {
          outcome: outcome("pending"),
          import: "planned",
          plan_id: planned.plan.plan_id,
          plan: planned.plan,
        };
      }
      return { outcome: outcome("committed"), import: "cancelled" };
    });
  });
  await page.route(/\/api\/v1\/imports\/[^/]+\/items/, (route) =>
    answer(route, () => api.imports.planItems("plan-1")),
  );
  await page.route(/\/api\/v1\/imports\/[^/]+\/start$/, (route) =>
    answer(route, async () => {
      const jobId = await api.imports.start("plan-1");
      const job = jobView({ id: jobId, state: "running", done: 0, total: PLAN.files_new });
      api.emit(job);
      return {
        outcome: { operation_id: "op-1", kind: "pending", error: null },
        import: "started",
        job_id: jobId,
      };
    }),
  );
  await page.route(/\/api\/v1\/imports\/[^/?]+$/, (route) =>
    route.request().method() === "DELETE"
      ? answer(route, async () => {
          await api.imports.discard("plan-1");
          return { plan_id: "plan-1", discarded: true };
        })
      : route.fallback(),
  );
}

/** Serve the job routes. */
async function serveJobRoutes(page: Page, api: ImportApi): Promise<void> {
  await page.route(/\/api\/v1\/jobs\?/, (route) => answer(route, () => api.jobs.list()));
  await page.route(/\/api\/v1\/jobs\/([^/]+)\/items\?/, (route) => {
    const job = /\/jobs\/([^/]+)\/items/.exec(route.request().url())?.[1] ?? "";
    return answer(route, () => api.jobs.items(decodeURIComponent(job)));
  });
  await page.route(/\/api\/v1\/jobs\/([^/]+)\/(pause|resume|cancel|retry-failed)$/, (route) => {
    const match = /\/jobs\/([^/]+)\/(pause|resume|cancel|retry-failed)$/.exec(
      route.request().url(),
    );
    const [job, action] = [decodeURIComponent(match?.[1] ?? ""), match?.[2] ?? ""];
    return answer(route, async () => {
      const view = await controlOf(api.jobs, action, job);
      return { job: view, scheduled: true };
    });
  });
  await page.route(/\/api\/v1\/jobs\/([^/]+)\/items\/([^/]+)\/retry$/, (route) => {
    const match = /\/jobs\/([^/]+)\/items\/([^/]+)\/retry$/.exec(route.request().url());
    return answer(route, async () => ({
      job: await api.jobs.retryItem(
        decodeURIComponent(match?.[1] ?? ""),
        decodeURIComponent(match?.[2] ?? ""),
      ),
      scheduled: true,
    }));
  });
}

/** Run one control on the memory job source. */
function controlOf(jobs: MemoryJobSource, action: string, job: string): Promise<JobView> {
  if (action === "pause") return jobs.pause(job);
  if (action === "resume") return jobs.resume(job);
  if (action === "cancel") return jobs.cancel(job);
  return jobs.retryFailed(job);
}

/**
 * Serve the whole fake host and return what it saw.
 *
 * @param page - The page under test.
 * @param options - What the import source answers and which jobs exist.
 * @returns The fake's handles.
 */
export async function serveImports(
  page: Page,
  options: ServeImportsOptions = {},
): Promise<ImportApi> {
  const sockets: WebSocketRoute[] = [];
  const jobs = new MemoryJobSource([...(options.jobs ?? [])], { ...(options.items ?? {}) });
  const imports = new MemoryImportSource({ plan: PLAN, items: ALL_PLAN_ITEMS, ...options.imports });
  const api: ImportApi = {
    imports,
    jobs,
    planBodies: [],
    lookups: [],
    planDelayMs: 0,
    emit: (job) => {
      const event = JSON.stringify({ topic: "job.update", data: job });
      jobs.jobs.splice(
        0,
        jobs.jobs.length,
        ...jobs.jobs.filter((known) => known.id !== job.id),
        job,
      );
      sockets.forEach((socket) => socket.send(event));
    },
  };
  jobs.subscribe((change) => {
    if (change.kind === "job") {
      const event = JSON.stringify({ topic: "job.update", data: change.job });
      sockets.forEach((socket) => socket.send(event));
    }
  });
  await page.route(/\/api\/v1\/host$/, (route) => route.fulfill({ json: DESKTOP }));
  await page.routeWebSocket(/\/api\/v1\/events$/, (socket) => void sockets.push(socket));
  await serveNavigation(page, TREE);
  await serveDrive(page, ROWS, undefined, ["l1", "c1", "c2", "c3", "f1", "f2", "f3", "v1"]);
  await page.route(/\/api\/v1\/navigation\/destination\?/, (route) => {
    const node = new URL(route.request().url()).searchParams.get("node") ?? "";
    api.lookups.push(node);
    const collection = COLLECTION_OF[node];
    if (collection === undefined) {
      return route.fulfill({ status: 404, json: errorBody("not_found") });
    }
    return route.fulfill({ json: { library_id: LIBRARY, collection_id: collection } });
  });
  await serveImportRoutes(page, api);
  await serveJobRoutes(page, api);
  return api;
}

/**
 * Every file of the plan of mockup 18: 214 files, the six the mockup draws first (five new, then
 * one already in the library) and the rest, so the plan's counts and its files agree: 196 new and
 * 18 already in the library.
 */
export const ALL_PLAN_ITEMS: readonly PlanItem[] = [
  ...PLAN_ITEMS.slice(0, 5),
  PLAN_ITEMS[5] as PlanItem,
  ...Array.from({ length: 208 }, (_, index) => ({
    item_id: `g${index}`,
    name: `IMG_${5000 + index}.JPG`,
    bytes: 6_000_000,
    present: index < 17,
    numbered: false,
  })),
];

/** A plan with other counts, for tests that need one. */
export function planWith(over: Partial<ImportPlan>): ImportPlan {
  return { ...PLAN, ...over };
}

/** Planned files for tests that need others. */
export function plannedFiles(names: readonly string[]): PlanItem[] {
  return names.map((name, i) => ({
    item_id: `p${i}`,
    name,
    bytes: 1000,
    present: false,
    numbered: false,
  }));
}
