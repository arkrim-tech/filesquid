/**
 * Unit tests for the library model (UI-1.1, API-S-025, API-S-023): the per-library routes
 * (`#/library`, `#/library/<id>`, `#/library/<id>/repair`), and the consent request built from
 * the repair candidates (1 to 1,000 distinct object IDs, each at most 1,024 bytes).
 */
import { afterEach, describe, expect, it, vi } from "vitest";
import {
  LIBRARY_ROUTE,
  REPAIR_STATEMENT_ID,
  consentRequest,
  libraryHref,
  libraryRepairHref,
  parseLibraryHash,
} from "../../src/index";
import { CANDIDATES, FIRST_CANDIDATE } from "../support/library-fakes";

afterEach(() => vi.restoreAllMocks());

describe("library routes", () => {
  it("names the start route and the per-library and repair routes", () => {
    expect(LIBRARY_ROUTE).toBe("#/library");
    expect(libraryHref("lib 1/é")).toBe("#/library/lib%201%2F%C3%A9");
    expect(libraryRepairHref("lib-1")).toBe("#/library/lib-1/repair");
  });

  it("parses every route and round-trips an ID that needs escaping", () => {
    expect(parseLibraryHash("#/library")).toEqual({ kind: "start" });
    expect(parseLibraryHash(libraryHref("lib 1/é"))).toEqual({ kind: "library", id: "lib 1/é" });
    expect(parseLibraryHash(libraryRepairHref("lib 1/é"))).toEqual({
      kind: "repair",
      id: "lib 1/é",
    });
  });

  it("ignores other hashes, empty IDs, other sub-routes and IDs over 128 bytes", () => {
    for (const hash of ["", "#/monitor", "#/libraryx", "#/library/", "#/library//repair"]) {
      expect(parseLibraryHash(hash), hash).toBeUndefined();
    }
    expect(parseLibraryHash("#/library/a/other")).toBeUndefined();
    expect(parseLibraryHash(`#/library/${"a".repeat(129)}`)).toBeUndefined();
    expect(parseLibraryHash(`#/library/${"a".repeat(128)}`)).toEqual({
      kind: "library",
      id: "a".repeat(128),
    });
  });

  it("logs and ignores an ID whose escapes do not decode", () => {
    const warn = vi.spyOn(console, "warn").mockImplementation(() => undefined);
    expect(parseLibraryHash("#/library/%E0%A4%A")).toBeUndefined();
    expect(warn).toHaveBeenCalledOnce();
  });
});

describe("consentRequest", () => {
  it("names exactly the listed objects in the request and in the consent", () => {
    const request = consentRequest("lib-1", CANDIDATES.candidates);
    const ids = ["urn:uuid:obj-a", "urn:uuid:obj-b"];
    expect(request).toEqual({
      library_id: "lib-1",
      object_ids: ids,
      consent: {
        library_id: "lib-1",
        object_ids: ids,
        statement_id: "version_inventory_repair.v2",
        confirmed: true,
      },
    });
    expect(REPAIR_STATEMENT_ID).toBe("version_inventory_repair.v2");
  });

  it("refuses an empty list, more than 1,000 objects and an ID over 1,024 bytes", () => {
    const one = FIRST_CANDIDATE;
    expect(consentRequest("lib-1", [])).toBeUndefined();
    const many = Array.from({ length: 1001 }, (_, i) => ({ ...one, object_id: `o${i}` }));
    expect(consentRequest("lib-1", many)).toBeUndefined();
    expect(consentRequest("lib-1", many.slice(0, 1000))?.object_ids).toHaveLength(1000);
    expect(consentRequest("lib-1", [{ ...one, object_id: "x".repeat(1025) }])).toBeUndefined();
    expect(consentRequest("lib-1", [{ ...one, object_id: "x".repeat(1024) }])).toBeDefined();
  });

  it("counts bytes, not characters, and sends each object once", () => {
    const one = FIRST_CANDIDATE;
    expect(consentRequest("lib-1", [{ ...one, object_id: "é".repeat(513) }])).toBeUndefined();
    const twice = consentRequest("lib-1", [one, one]);
    expect(twice?.object_ids).toEqual([one.object_id]);
    expect(twice?.consent?.object_ids).toEqual([one.object_id]);
  });
});
