/**
 * A fake of the navigation listing for the production-build Playwright tests (UI-F2.1b part
 * 2): the page is the real built UI, `GET /api/v1/navigation` is answered here from a
 * `MemoryNavigationSource`, which answers as the API does (API-S-021: repository at the root,
 * one level per call, `ancestors`, opaque cursors, `not_found`). It records each call's node,
 * cursor and limit, can page in small pages to exercise cursor paging, and can answer 503
 * `unavailable` as the desktop host does until it installs the navigation service.
 */
import type { Page } from "@playwright/test";
import { ApiError } from "../../../src/api/client";
import { MemoryNavigationSource, type FixtureNode } from "../../../src/navigation/source";

/** One library with collections (one holding folders), a virtual collection and a folder. */
export const FAMILY: FixtureNode = {
  node: "r",
  kind: "repository",
  name: null,
  children: [
    {
      node: "l1",
      kind: "library",
      name: "Family",
      children: [
        {
          node: "c1",
          kind: "collection",
          name: "Photos",
          children: [
            { node: "f1", kind: "folder", name: "2025" },
            { node: "f2", kind: "folder", name: "Archive" },
          ],
        },
        { node: "c2", kind: "collection", name: "Hobbies" },
        { node: "v1", kind: "virtual_collection", name: "Best of 2025" },
        { node: "f9", kind: "folder", name: "inbox" },
      ],
    },
  ],
};

/** One listing call as the page sent it. */
export interface NavigationCall {
  /** The `node` parameter, or null for the repository level. */
  readonly node: string | null;
  /** The `cursor` parameter, or null for a first page. */
  readonly cursor: string | null;
  /** The `limit` parameter, or null when the page left it to the default. */
  readonly limit: string | null;
}

/** What the fake saw and how to steer it. */
export interface NavigationApi {
  /** The tree the fake serves; `replace` it to change the tree. */
  readonly source: MemoryNavigationSource;
  /** Every listing call, in order. */
  readonly calls: NavigationCall[];
  /** Answer 503 `unavailable` while true. */
  unavailable: boolean;
}

/**
 * Answer `GET /api/v1/navigation` from a fixture tree.
 *
 * @param page - The page under test.
 * @param root - The tree to serve (default {@link FAMILY}).
 * @param pageSize - Page size used when the page sends no `limit` (default 200, the API's).
 * @returns What the fake saw, and the switch for the 503 answer.
 */
export async function serveNavigation(
  page: Page,
  root: FixtureNode = FAMILY,
  pageSize = 200,
): Promise<NavigationApi> {
  const api: NavigationApi = {
    source: new MemoryNavigationSource(root),
    calls: [],
    unavailable: false,
  };
  await page.route(/\/api\/v1\/navigation(\?|$)/, async (route) => {
    const query = new URL(route.request().url()).searchParams;
    const call = {
      node: query.get("node"),
      cursor: query.get("cursor"),
      limit: query.get("limit"),
    };
    api.calls.push(call);
    if (api.unavailable) {
      const body = { code: "unavailable", message: "not installed", retryable: true };
      await route.fulfill({ status: 503, json: body });
      return;
    }
    try {
      const limit = call.limit === null ? pageSize : Number(call.limit);
      await route.fulfill({ json: await api.source.list(call.node, call.cursor, limit) });
    } catch (error) {
      const status = error instanceof ApiError ? error.status : 500;
      const code = error instanceof ApiError ? (error.code ?? "internal") : "internal";
      await route.fulfill({ status, json: { code, message: "refused", retryable: false } });
    }
  });
  return api;
}
