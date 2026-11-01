/**
 * Unit tests for the hash routes of the rebuild job page (UI-1.3, UI-S-027): `#/library/<id>/
 * recover/<job>` after Recover (mockup 04) and `#/library/<id>/rebuild/<job>` after the library
 * page's Rebuild. IDs are escaped and bounded like the library IDs of the other routes.
 */
import { describe, expect, it } from "vitest";
import {
  libraryRebuildHref,
  libraryRecoverHref,
  parseLibraryHash,
} from "../../src/libraries/model";

describe("rebuild job routes", () => {
  it("builds and reads the recover address", () => {
    const href = libraryRecoverHref("lib 1", "job/9");
    expect(href).toBe("#/library/lib%201/recover/job%2F9");
    expect(parseLibraryHash(href)).toEqual({
      kind: "job",
      mode: "recover",
      id: "lib 1",
      job: "job/9",
    });
  });

  it("builds and reads the rebuild address", () => {
    const href = libraryRebuildHref("lib-1", "job-1");
    expect(href).toBe("#/library/lib-1/rebuild/job-1");
    expect(parseLibraryHash(href)).toEqual({
      kind: "job",
      mode: "rebuild",
      id: "lib-1",
      job: "job-1",
    });
  });

  it.each([
    "#/library/lib-1/rebuild",
    "#/library/lib-1/rebuild/",
    "#/library/lib-1/rebuild/job-1/extra",
    "#/library/lib-1/other/job-1",
    "#/library//rebuild/job-1",
    `#/library/lib-1/rebuild/${"j".repeat(129)}`,
    `#/library/lib-1/rebuild/${"é".repeat(65)}`,
    "#/library/lib-1/rebuild/%E0%A4%A",
  ])("does not read %s as a job page", (hash) => {
    expect(parseLibraryHash(hash)).toBeUndefined();
  });

  it("still reads the existing library and repair routes", () => {
    expect(parseLibraryHash("#/library/lib-1")).toEqual({ kind: "library", id: "lib-1" });
    expect(parseLibraryHash("#/library/lib-1/repair")).toEqual({ kind: "repair", id: "lib-1" });
    expect(parseLibraryHash("#/library")).toEqual({ kind: "start" });
  });
});
