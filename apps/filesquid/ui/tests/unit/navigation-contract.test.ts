/**
 * UI-F2.1b part 2: every navigation answer and `navigation.change` payload is checked against
 * the API-S-021 contract before the tree or breadcrumb shows it (opaque handles of at most 512
 * bytes, the five kinds, a `null` name only for a repository, `has_children`, `ancestors`,
 * `next_cursor`; exactly one of `parents` (1 to 64 handles) and `all`).
 */
import { describe, expect, it } from "vitest";
import {
  MAX_CHANGE_PARENTS,
  MAX_PAGE_NODES,
  parseNavigationChange,
  parseNavigationPage,
} from "../../src/navigation/contract";

const repository = { node: "r", kind: "repository", name: null, has_children: true };
const library = { node: "l1", kind: "library", name: "Family", has_children: true };
const folder = { node: "f1", kind: "folder", name: "2025", has_children: false };

/** A page with overrides. */
const page = (over: Record<string, unknown> = {}) => ({
  nodes: [folder],
  ancestors: [repository, library],
  next_cursor: null,
  ...over,
});

describe("parseNavigationPage", () => {
  it("accepts a page that satisfies the contract and returns its nodes, ancestors and cursor", () => {
    expect(parseNavigationPage(page({ next_cursor: "c2" }))).toEqual({
      nodes: [folder],
      ancestors: [repository, library],
      next_cursor: "c2",
    });
  });

  it("accepts the repository level: one repository with a null name and no ancestors", () => {
    expect(parseNavigationPage({ nodes: [repository], ancestors: [], next_cursor: null })).toEqual({
      nodes: [repository],
      ancestors: [],
      next_cursor: null,
    });
  });

  it("keeps only the contract fields of a node, so nothing else reaches the DOM", () => {
    const parsed = parseNavigationPage(page({ nodes: [{ ...folder, path: "/secret" }] }));
    expect(parsed?.nodes[0]).toEqual(folder);
  });

  it("accepts a handle of exactly 512 bytes and refuses one of 513", () => {
    const at = "é".repeat(256);
    expect(parseNavigationPage(page({ nodes: [{ ...folder, node: at }] }))).toBeDefined();
    const over = `${at}x`;
    expect(parseNavigationPage(page({ nodes: [{ ...folder, node: over }] }))).toBeUndefined();
  });

  it.each([
    ["an empty handle", { ...folder, node: "" }],
    ["a numeric handle", { ...folder, node: 7 }],
    ["an unknown kind", { ...folder, kind: "file" }],
    ["a null name on a folder", { ...folder, name: null }],
    ["a numeric name", { ...folder, name: 3 }],
    ["has_children that is not a boolean", { ...folder, has_children: "yes" }],
    ["a node that is not an object", "f1"],
  ])("refuses a page with %s", (_case, node) => {
    expect(parseNavigationPage(page({ nodes: [node] }))).toBeUndefined();
  });

  it.each([
    ["no nodes field", { ancestors: [], next_cursor: null }],
    ["nodes that are not a list", page({ nodes: {} })],
    ["ancestors that are not a list", page({ ancestors: null })],
    ["a bad ancestor", page({ ancestors: [{ ...library, kind: "shelf" }] })],
    ["an empty cursor", page({ next_cursor: "" })],
    ["a numeric cursor", page({ next_cursor: 2 })],
    ["no cursor field", { nodes: [], ancestors: [] }],
    ["a value that is not an object", []],
  ])("refuses %s", (_case, value) => {
    expect(parseNavigationPage(value)).toBeUndefined();
  });

  it(`refuses more than ${MAX_PAGE_NODES} nodes on one page (the largest limit)`, () => {
    const many = Array.from({ length: MAX_PAGE_NODES + 1 }, (_, i) => ({
      ...folder,
      node: `f${i}`,
    }));
    expect(parseNavigationPage(page({ nodes: many }))).toBeUndefined();
    expect(parseNavigationPage(page({ nodes: many.slice(1) }))).toBeDefined();
  });
});

describe("parseNavigationChange", () => {
  it("accepts the named parents", () => {
    expect(parseNavigationChange({ parents: ["l1", "f1"] })).toEqual({ parents: ["l1", "f1"] });
  });

  it("accepts all", () => {
    expect(parseNavigationChange({ all: true })).toEqual({ all: true });
  });

  it(`accepts ${MAX_CHANGE_PARENTS} parents and refuses ${MAX_CHANGE_PARENTS + 1}`, () => {
    const parents = Array.from({ length: MAX_CHANGE_PARENTS + 1 }, (_, i) => `n${i}`);
    expect(parseNavigationChange({ parents: parents.slice(1) })).toBeDefined();
    expect(parseNavigationChange({ parents })).toBeUndefined();
  });

  it.each([
    ["both fields", { parents: ["l1"], all: true }],
    ["neither field", {}],
    ["no parents", { parents: [] }],
    ["a bad handle", { parents: ["l1", ""] }],
    ["all false", { all: false }],
    ["a value that is not an object", "all"],
    ["null", null],
  ])("refuses %s", (_case, value) => {
    expect(parseNavigationChange(value)).toBeUndefined();
  });
});
