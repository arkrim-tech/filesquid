/**
 * Unit tests for the library failure mapping (UI-1.1, API-S-034, API-S-025): a 503 is the
 * unavailable state (with its reason for the folder chooser), an authorisation denial is the
 * denied state, conflicts, missing locations and invalid requests are messages in the active
 * labels, and an unknown failure is offline or error.
 */
import { describe, expect, it } from "vitest";
import { libraryFailure, LibraryRegistry } from "../../src/index";
import labels from "../../src/theme/labels.en.json";
import { LIBRARY, apiError } from "../support/library-fakes";

describe("libraryFailure", () => {
  it("shows a 503 from the folder chooser as unavailable with its reason", () => {
    expect(libraryFailure(apiError(503, "unavailable"), "choose", labels, true)).toEqual({
      kind: "state",
      state: { kind: "unavailable", reason: labels.libraryChooserUnavailable },
    });
  });

  it("shows a 503 from another library route as unavailable without that reason", () => {
    expect(libraryFailure(apiError(503, "unavailable"), "create", labels, true)).toEqual({
      kind: "state",
      state: { kind: "unavailable" },
    });
  });

  it("shows an authorisation denial as the denied state with its reason", () => {
    expect(
      libraryFailure(apiError(403, "forbidden", "not_permitted"), "open", labels, true),
    ).toEqual({ kind: "state", state: { kind: "denied", reason: "not_permitted" } });
  });

  it("says only the owner can repair when the repair is denied as not permitted", () => {
    expect(
      libraryFailure(apiError(403, "forbidden", "not_permitted"), "repair", labels, true),
    ).toEqual({ kind: "message", text: labels.repairRequestRefusedOwner });
  });

  it.each([
    ["choose", labels.libraryConflictChoice],
    ["create", labels.libraryConflictCreate],
    ["open", labels.libraryConflictOpen],
    ["copy", labels.libraryConflictCopy],
    ["repair", labels.repairRequestRefusedConflict],
  ] as const)("words a 409 from %s", (action, text) => {
    expect(libraryFailure(apiError(409, "conflict"), action, labels, true)).toEqual({
      kind: "message",
      text,
    });
  });

  it("words 404 and 400 and tells a missing library from a missing folder choice", () => {
    expect(libraryFailure(apiError(404, "not_found"), "open", labels, true)).toEqual({
      kind: "message",
      text: labels.libraryNotFound,
    });
    expect(libraryFailure(apiError(404, "not_found"), "repair", labels, true)).toEqual({
      kind: "message",
      text: labels.repairRequestRefusedGone,
    });
    expect(libraryFailure(apiError(400, "invalid_request"), "create", labels, true)).toEqual({
      kind: "message",
      text: labels.libraryInvalid,
    });
  });

  it("words a repair 503 and an unexpected status for the repair", () => {
    expect(libraryFailure(apiError(503, "unavailable"), "repair", labels, true)).toEqual({
      kind: "message",
      text: labels.repairRequestRefusedUnavailable,
    });
    expect(libraryFailure(apiError(500, "internal"), "repair", labels, true)).toEqual({
      kind: "message",
      text: labels.repairRequestRefused,
    });
  });

  it("is offline while the browser is offline and an error otherwise", () => {
    expect(libraryFailure(new TypeError("fetch failed"), "open", labels, false)).toEqual({
      kind: "state",
      state: { kind: "offline" },
    });
    expect(libraryFailure(new TypeError("bad"), "open", labels, true)).toEqual({
      kind: "state",
      state: { kind: "error" },
    });
    expect(libraryFailure(apiError(500, "internal"), "open", labels, true)).toEqual({
      kind: "message",
      text: labels.libraryFailed,
    });
  });
});

describe("LibraryRegistry", () => {
  it("remembers the latest view of a library", () => {
    const registry = new LibraryRegistry();
    expect(registry.get("lib-1")).toBeUndefined();
    registry.remember(LIBRARY);
    expect(registry.get("lib-1")).toEqual(LIBRARY);
    registry.remember({ ...LIBRARY, objects: 9 });
    expect(registry.get("lib-1")?.objects).toBe(9);
  });

  it("keeps at most 64 libraries, dropping the oldest", () => {
    const registry = new LibraryRegistry();
    for (let i = 0; i < 70; i += 1) registry.remember({ ...LIBRARY, library_id: `l${i}` });
    expect(registry.get("l0")).toBeUndefined();
    expect(registry.get("l5")).toBeUndefined();
    expect(registry.get("l6")).toBeDefined();
    expect(registry.get("l69")).toBeDefined();
    expect(registry.size).toBe(64);
  });
});
