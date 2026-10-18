// @vitest-environment node
/**
 * F-004 default UI and UI-S-017 (UI-F2.1a): the pure navigation model behind `<fs-nav-tree>`
 * and `<fs-breadcrumb>`: a stored value is accepted only when it is version 1, has no unknown
 * field, holds `{node, kind}` pins and handles only, and keeps every bound; section order and
 * moves; bounded lists; display names; routes; the breadcrumb from `ancestors` (API-S-021).
 */
import { describe, expect, it } from "vitest";
import { ApiError } from "../../src/api/client";
import {
  EMPTY_NAVIGATION_PREFERENCES,
  MAX_COLLAPSED,
  MAX_DEPTH,
  MAX_HANDLE_BYTES,
  MAX_PINNED,
  breadcrumbItems,
  isHandle,
  isNotFound,
  moveSection,
  navigationHref,
  nodeName,
  readStoredPreferences,
  sectionOrder,
  serializeNavigationPreferences,
  unavailableName,
  withMember,
  type NavigationNode,
} from "../../src/navigation/model";
import { formatLabel } from "../../src/theme/labels";
import labels from "../../src/theme/labels.en.json";

const repo: NavigationNode = { node: "r", kind: "repository", name: null, has_children: true };
const family: NavigationNode = { node: "l1", kind: "library", name: "Family", has_children: true };
const photos: NavigationNode = {
  node: "c1",
  kind: "collection",
  name: "Photos",
  has_children: true,
};

describe("readStoredPreferences (UI-S-017)", () => {
  it("accepts a version 1 value of pins, section order and collapsed handles", () => {
    const value = {
      version: 1,
      pinned: [{ node: "c1", kind: "collection" }],
      order: ["virtual_collection", "collection"],
      collapsed: ["l1"],
    };
    expect(readStoredPreferences(value)).toEqual({
      ok: true,
      prefs: { pinned: value.pinned, order: value.order, collapsed: value.collapsed },
    });
  });

  it("treats missing lists as empty", () => {
    expect(readStoredPreferences({ version: 1 })).toEqual({
      ok: true,
      prefs: EMPTY_NAVIGATION_PREFERENCES,
    });
  });

  it.each([
    ["not an object", 3, /not an object/],
    ["an array", [], /not an object/],
    ["another version", { version: 2 }, /version is not 1/],
    ["an unknown field", { version: 1, theme: "dark" }, /unknown fields \(theme\)/],
    ["a stored name", { version: 1, pinned: [{ node: "a", kind: "folder", name: "x" }] }, /pins/],
    ["an unknown kind", { version: 1, pinned: [{ node: "a", kind: "album" }] }, /pins/],
    ["an empty handle", { version: 1, collapsed: [""] }, /collapsed/],
    ["explicit null pins", { version: 1, pinned: null }, /pins/],
    ["an explicit null order", { version: 1, order: null }, /section order/],
    ["explicit null collapsed nodes", { version: 1, collapsed: null }, /collapsed/],
    ["a non-string order entry", { version: 1, order: [3] }, /section order/],
  ])("refuses %s with a reason", (_case, value, reason) => {
    const result = readStoredPreferences(value);
    expect(result.ok).toBe(false);
    expect(result.ok ? "" : result.problem).toMatch(reason);
  });

  it("refuses a value over a bound instead of truncating it", () => {
    const pins = Array.from({ length: MAX_PINNED + 1 }, (_, i) => ({
      node: `n${i}`,
      kind: "folder",
    }));
    expect(readStoredPreferences({ version: 1, pinned: pins }).ok).toBe(false);
    const handles = Array.from({ length: MAX_COLLAPSED + 1 }, (_, i) => `h${i}`);
    expect(readStoredPreferences({ version: 1, collapsed: handles }).ok).toBe(false);
    const atBound = Array.from({ length: MAX_COLLAPSED }, (_, i) => `h${i}`);
    expect(readStoredPreferences({ version: 1, collapsed: atBound }).ok).toBe(true);
  });

  it("refuses a handle longer than 512 UTF-8 bytes", () => {
    const wide = "é".repeat(MAX_HANDLE_BYTES / 2 + 1);
    expect(isHandle("é".repeat(MAX_HANDLE_BYTES / 2))).toBe(true);
    expect(isHandle(wide)).toBe(false);
    expect(readStoredPreferences({ version: 1, collapsed: [wide] }).ok).toBe(false);
  });

  it("serialises only handles, kinds and section ids, with the version", () => {
    const forbidden = {
      name: "Photos",
      title: "Summer title",
      logical_path: "2025/summer/beach.jpg",
      token: "secret-bearer-token",
      content_handle: "content-handle-123",
    };
    const pin = { node: "c1", kind: "collection", ...forbidden } as never;
    const text = serializeNavigationPreferences({
      pinned: [pin],
      order: ["folder"],
      collapsed: ["l1"],
    });
    expect(text).toBe(
      '{"version":1,"pinned":[{"node":"c1","kind":"collection"}],"order":["folder"],"collapsed":["l1"]}',
    );
    for (const value of Object.values(forbidden)) expect(text).not.toContain(value);
  });
});

describe("sectionOrder and moveSection", () => {
  it("keeps the default order when nothing is saved", () => {
    expect(sectionOrder([], ["pinned", "collection"])).toEqual(["pinned", "collection"]);
  });

  it("follows the saved order, ignoring ids that are not shown", () => {
    expect(sectionOrder(["folder", "gone", "collection"], ["collection", "folder"])).toEqual([
      "folder",
      "collection",
    ]);
  });

  it("leads with new sections that precede every saved one, and appends the others", () => {
    const ids = ["pinned", "collection", "virtual_collection", "folder"];
    expect(sectionOrder(["virtual_collection", "collection"], ids)).toEqual([
      "pinned",
      "virtual_collection",
      "collection",
      "folder",
    ]);
  });

  it("moves a section one step and stops at either end", () => {
    const order = ["a", "b", "c"];
    expect(moveSection(order, "b", -1)).toEqual(["b", "a", "c"]);
    expect(moveSection(order, "b", 1)).toEqual(["a", "c", "b"]);
    expect(moveSection(order, "a", -1)).toEqual(order);
    expect(moveSection(order, "c", 1)).toEqual(order);
    expect(moveSection(order, "zz", 1)).toEqual(order);
  });
});

describe("withMember", () => {
  it("adds at the end, removes, and leaves the list alone when nothing changes", () => {
    expect(withMember(["a"], "b", true, 3)).toEqual(["a", "b"]);
    expect(withMember(["a", "b"], "a", false, 3)).toEqual(["b"]);
    expect(withMember(["a"], "a", true, 3)).toEqual(["a"]);
  });

  it("drops the oldest entry at the bound", () => {
    expect(withMember(["a", "b", "c"], "d", true, 3)).toEqual(["b", "c", "d"]);
  });
});

describe("names, routes and the breadcrumb", () => {
  it("shows a node's name, or its kind's label when it has none", () => {
    expect(nodeName(photos, labels)).toBe("Photos");
    expect(nodeName(repo, labels)).toBe(labels.repository);
  });

  it("routes to the URI-encoded handle", () => {
    expect(navigationHref({ node: "a/b c" })).toBe("#/navigation/a%2Fb%20c");
  });

  it("builds the breadcrumb from ancestors, repository first", () => {
    expect(breadcrumbItems([repo, family, photos], labels)).toEqual([
      { node: "r", name: labels.repository, href: "#/navigation/r" },
      { node: "l1", name: "Family", href: "#/navigation/l1" },
      { node: "c1", name: "Photos", href: "#/navigation/c1" },
    ]);
    expect(breadcrumbItems([photos], labels, (n) => `#/x/${n.node}`)[0]?.href).toBe("#/x/c1");
  });

  it("caps the breadcrumb at MAX_DEPTH, keeping the current node last", () => {
    const deep = Array.from({ length: MAX_DEPTH + 3 }, (_, i) => ({ ...photos, node: `n${i}` }));
    const trail = breadcrumbItems(deep, labels);
    expect(trail).toHaveLength(MAX_DEPTH);
    expect(trail.at(-1)?.node).toBe(`n${MAX_DEPTH + 2}`);
    expect(trail[0]?.node).toBe("n3");
  });

  it("names an unavailable pin by its kind", () => {
    expect(unavailableName({ node: "c9", kind: "collection" }, labels)).toBe(
      formatLabel(labels.navigationUnavailable, { kind: labels.collection }),
    );
  });

  it("recognises a not_found answer", () => {
    expect(isNotFound(new ApiError(404, "/api/v1/navigation", { code: "not_found" }))).toBe(true);
    expect(isNotFound(new ApiError(403, "/api/v1/navigation", { code: "forbidden" }))).toBe(false);
    expect(isNotFound(new Error("not_found"))).toBe(false);
  });
});

describe("formatLabel", () => {
  it("fills named placeholders and leaves unknown ones", () => {
    expect(formatLabel("Move {name} up", { name: "Holidays" })).toBe("Move Holidays up");
    expect(formatLabel("{name} and {name}", { name: "x" })).toBe("x and x");
    expect(formatLabel("Keep {other}", { name: "x" })).toBe("Keep {other}");
    expect(formatLabel("Keep {constructor}", {})).toBe("Keep {constructor}");
  });

  it("inserts a value literally, even when it looks like a placeholder or a pattern", () => {
    expect(formatLabel("Pin {name}", { name: "$& {name}" })).toBe("Pin $& {name}");
  });
});
