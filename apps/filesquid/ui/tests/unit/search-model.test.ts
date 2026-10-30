/**
 * Unit tests for the search model (UI-F2.9, F-063, UI-S-007, UI-S-030, API-S-012): how chips and
 * text become the query the search operation takes, how a route's query becomes chips again, how
 * the host's refusals are told apart and name their operator, and the bounds on every list.
 */
import { describe, expect, it } from "vitest";
import { ApiError } from "../../src/api/client";
import {
  MAX_CHIPS,
  MAX_QUERY_BYTES,
  OPERATORS_ON_HOST,
  addChip,
  chipLabel,
  composeQuery,
  modifiedAfter,
  parseQuery,
  refusalOf,
  removeChip,
  tokenizeQuery,
  typeClassOf,
  typeSuggestions,
  zoneOf,
  type SearchChip,
} from "../../src/search/model";
import labels from "../../src/theme/labels.en.json";

/** 12:00 UTC on 16 October 2026, the clock of the other drive tests. */
const NOW = new Date("2026-10-16T12:00:00Z");

describe("composeQuery", () => {
  it("puts the chip terms first and the text after them", () => {
    const chips: SearchChip[] = [
      { filter: "type", value: "document" },
      { filter: "modified", value: "month" },
    ];
    expect(composeQuery(chips, "harbour", NOW)).toBe("type:document after:2026-09-16 harbour");
  });

  it("quotes a value that holds a space or a colon and drops its quotes", () => {
    const chips: SearchChip[] = [
      { filter: "in", value: "Harbour project" },
      { filter: "title", value: 'say "hi": now' },
    ];
    expect(composeQuery(chips, "", NOW)).toBe('in:"Harbour project" title:"say hi: now"');
  });

  it("keeps a chip term whole or leaves it out, never cut through its quotes", () => {
    const chips: SearchChip[] = [
      { filter: "title", value: "a ".repeat(600) + "b" },
      { filter: "type", value: "image" },
    ];
    expect(composeQuery(chips, "harbour", NOW)).toBe("type:image harbour");
  });

  it("drops a quoted phrase the byte limit would cut, so no quote is left open", () => {
    const text = "w ".repeat(480) + '"exact phrase that is cut here and there"';
    const query = composeQuery([], text, NOW);
    expect(new TextEncoder().encode(query).length).toBeLessThanOrEqual(MAX_QUERY_BYTES);
    expect([...query].filter((c) => c === '"').length % 2).toBe(0);
  });

  it("trims the text and gives an empty query for no chips and no text", () => {
    expect(composeQuery([], "  harbour  plan ", NOW)).toBe("harbour  plan");
    expect(composeQuery([], "   ", NOW)).toBe("");
  });

  it("never exceeds the 1,024 bytes the API accepts, cutting at a character", () => {
    const text = "é".repeat(2000);
    const query = composeQuery([{ filter: "type", value: "image" }], text, NOW);
    expect(new TextEncoder().encode(query).length).toBeLessThanOrEqual(MAX_QUERY_BYTES);
    expect(query.startsWith("type:image é")).toBe(true);
    expect(query.endsWith("é")).toBe(true);
  });
});

describe("modifiedAfter and zoneOf", () => {
  it("counts the days back from the caller's calendar date", () => {
    expect(modifiedAfter("today", NOW)).toBe("2026-10-16");
    expect(modifiedAfter("week", NOW)).toBe("2026-10-09");
    expect(modifiedAfter("month", NOW)).toBe("2026-09-16");
    expect(modifiedAfter("year", NOW)).toBe("2025-10-16");
  });

  it("words the caller's zone as Z or a fixed offset within the contract's range", () => {
    expect(zoneOf(NOW)).toBe("Z");
    const offsets = [
      [-480, "+08:00"],
      [480, "-08:00"],
      [-345, "+05:45"],
      [0, "Z"],
    ] as const;
    for (const [minutes, text] of offsets) {
      const date = new Date(NOW);
      date.getTimezoneOffset = () => minutes;
      expect(zoneOf(date)).toBe(text);
    }
  });
});

describe("addChip and removeChip", () => {
  it("keeps one Type, one Location and one Modified chip, the newest replacing the older", () => {
    let chips: SearchChip[] = [];
    chips = addChip(chips, { filter: "type", value: "image" });
    chips = addChip(chips, { filter: "type", value: "document" });
    chips = addChip(chips, { filter: "in", value: "Photos" });
    chips = addChip(chips, { filter: "modified", value: "week" });
    expect(chips).toEqual([
      { filter: "type", value: "document" },
      { filter: "in", value: "Photos" },
      { filter: "modified", value: "week" },
    ]);
  });

  it("allows several Title chips but not the same one twice, and stops at the chip bound", () => {
    let chips: SearchChip[] = [];
    for (let index = 0; index < MAX_CHIPS + 4; index += 1) {
      chips = addChip(chips, { filter: "title", value: `word${index}` });
    }
    chips = addChip(chips, { filter: "title", value: "word0" });
    expect(chips).toHaveLength(MAX_CHIPS);
    expect(chips.map((chip) => chip.value)).toEqual(
      Array.from({ length: MAX_CHIPS }, (_, index) => `word${index}`),
    );
  });

  it("removes one chip by its place and ignores a place that is not there", () => {
    const chips: SearchChip[] = [
      { filter: "type", value: "image" },
      { filter: "in", value: "Photos" },
    ];
    expect(removeChip(chips, 0)).toEqual([{ filter: "in", value: "Photos" }]);
    expect(removeChip(chips, 5)).toEqual(chips);
  });
});

describe("chipLabel", () => {
  it("words a chip from the label table", () => {
    expect(chipLabel({ filter: "type", value: "document" }, labels)).toBe("Type: Document");
    expect(chipLabel({ filter: "modified", value: "month" }, labels)).toBe(
      "Modified: last 30 days",
    );
    expect(chipLabel({ filter: "in", value: "Harbour project" }, labels)).toBe(
      "Location: Harbour project",
    );
    expect(chipLabel({ filter: "title", value: "harbour" }, labels)).toBe("Title: harbour");
  });
});

describe("tokenizeQuery", () => {
  it("splits on spaces outside quotes and recognises operators as the API does", () => {
    const tokens = tokenizeQuery('type:IMAGE -"old copy" size>10MB creator:"Ana Example" plain');
    expect(tokens.map((t) => [t.operator, t.value, t.negated])).toEqual([
      ["type", "IMAGE", false],
      [undefined, "old copy", true],
      ["size>", "10MB", false],
      ["creator", "Ana Example", false],
      [undefined, "plain", false],
    ]);
  });

  it("treats a colon inside quotes or after a non-letter as text", () => {
    const tokens = tokenizeQuery('"10:30" a1:b -x:y');
    expect(tokens.map((t) => t.operator)).toEqual([undefined, undefined, "x"]);
    expect(tokens[2]?.negated).toBe(true);
  });

  it("is bounded: a long query yields at most 32 tokens", () => {
    expect(tokenizeQuery(Array.from({ length: 100 }, () => "a").join(" "))).toHaveLength(32);
  });
});

describe("parseQuery", () => {
  it("turns leading Type, Location, Modified and Title terms back into chips", () => {
    const parsed = parseQuery(
      'type:document in:"Harbour project" after:2026-09-16 title:harbour plan -draft',
      NOW,
    );
    expect(parsed.chips).toEqual([
      { filter: "type", value: "document" },
      { filter: "in", value: "Harbour project" },
      { filter: "modified", value: "month" },
      { filter: "title", value: "harbour" },
    ]);
    expect(parsed.text).toBe("plan -draft");
  });

  it("leaves an operator the chips do not stand for in the text, as typed", () => {
    const parsed = parseQuery('harbour sharedwith:me after:2020-01-01 "exact phrase"', NOW);
    expect(parsed.chips).toEqual([]);
    expect(parsed.text).toBe('harbour sharedwith:me after:2020-01-01 "exact phrase"');
  });

  it("round-trips: composing what was parsed asks for the same terms", () => {
    const query = 'type:image in:"Site photos" after:2026-10-16 harbour -draft';
    const parsed = parseQuery(query, NOW);
    expect(composeQuery(parsed.chips, parsed.text, NOW)).toBe(query);
  });

  it("keeps every term of a query longer than the 32-term bound", () => {
    const words = Array.from({ length: 40 }, (_, i) => "w" + i).join(" ");
    const query = "type:image " + words;
    const parsed = parseQuery(query, NOW);
    expect(composeQuery(parsed.chips, parsed.text, NOW)).toBe(query);
  });

  it("keeps a second Type term in the text so nothing is dropped", () => {
    const parsed = parseQuery("type:image type:video", NOW);
    expect(parsed.chips).toEqual([{ filter: "type", value: "image" }]);
    expect(parsed.text).toBe("type:video");
  });
});

describe("refusalOf", () => {
  const unavailable = new ApiError(503, "/api/v1/files/search", { code: "unavailable" });
  const invalid = new ApiError(400, "/api/v1/files/search", { code: "invalid_request" });

  it("names the operator of an unavailable refusal as the host cannot answer it", () => {
    expect(refusalOf(unavailable, "harbour sharedwith:me")).toEqual({
      kind: "unavailable",
      operator: "sharedwith:",
    });
    expect(refusalOf(unavailable, "is:favorite")).toEqual({
      kind: "unavailable",
      operator: "is:favorite",
    });
    expect(refusalOf(unavailable, "owner:ben")).toEqual({
      kind: "unavailable",
      operator: "owner:",
    });
    for (const operator of ["to", "from", "pendingowner", "sharedwith"]) {
      expect(refusalOf(unavailable, operator + ":ben")).toEqual({
        kind: "unavailable",
        operator: operator + ":",
      });
    }
  });

  it("names the operator of an invalid request that is not in the grammar", () => {
    expect(refusalOf(invalid, "harbour colour:red")).toEqual({
      kind: "unknown",
      operator: "colour:",
    });
    expect(refusalOf(invalid, "generator:x")?.operator).toBe("generator:");
    expect(refusalOf(invalid, "is:unfiled")).toEqual({ kind: "unknown", operator: "is:unfiled" });
  });

  it("names no operator when a bound or an open quote is what was refused", () => {
    expect(refusalOf(invalid, 'colour:red "open')).toBeUndefined();
    expect(refusalOf(invalid, "colour:red " + "x".repeat(1100))).toBeUndefined();
    const many = Array.from({ length: 33 }, (_, i) => "w" + i).join(" ");
    expect(refusalOf(invalid, many + " colour:red")).toBeUndefined();
  });

  it("matches operator names without case", () => {
    expect(refusalOf(unavailable, "SharedWith:me")?.operator).toBe("sharedwith:");
  });

  it("is undefined for a refusal that names no operator and for any other failure", () => {
    expect(refusalOf(invalid, 'type:image "open')).toBeUndefined();
    expect(refusalOf(unavailable, "harbour")).toBeUndefined();
    expect(refusalOf(new Error("network"), "colour:red")).toBeUndefined();
    expect(
      refusalOf(new ApiError(500, "/api/v1/files/search", { code: "internal" }), "colour:red"),
    ).toBeUndefined();
  });
});

describe("operator list and type classes", () => {
  it("offers the ten operators of the mockups and none of the server's", () => {
    expect(OPERATORS_ON_HOST).toEqual([
      '"exact phrase"',
      "-word",
      "type:",
      "in:",
      "title:",
      "creator:",
      "before:",
      "after:",
      "size>",
      "is:deleted",
    ]);
    const joined = OPERATORS_ON_HOST.join(" ");
    for (const server of ["sharedwith", "to:", "from:", "pendingowner", "unfiled"]) {
      expect(joined).not.toContain(server);
    }
  });

  it("maps a media type to the API's class", () => {
    expect(typeClassOf("application/pdf")).toBe("document");
    expect(
      typeClassOf("application/vnd.openxmlformats-officedocument.wordprocessingml.document"),
    ).toBe("document");
    expect(typeClassOf("image/jpeg")).toBe("image");
    expect(typeClassOf("video/mp4")).toBe("video");
    expect(typeClassOf("audio/mp4")).toBe("audio");
    expect(typeClassOf("text/plain")).toBe("text");
    expect(typeClassOf("application/octet-stream")).toBeUndefined();
  });

  it.each([
    "application/pdf",
    "application/rtf",
    "application/msword",
    "application/vnd.ms-excel",
    "application/vnd.ms-powerpoint",
    "application/vnd.ms-word.document.macroEnabled.12",
    "application/vnd.ms-excel.sheet.macroEnabled.12",
    "application/vnd.ms-powerpoint.presentation.macroEnabled.12",
    "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
    "application/vnd.openxmlformats-officedocument.presentationml.presentation",
    "application/vnd.oasis.opendocument.text",
    "application/vnd.oasis.opendocument.spreadsheet",
    "application/vnd.oasis.opendocument.presentation",
  ])("classes %s as a document", (mediaType) => {
    expect(typeClassOf(mediaType)).toBe("document");
  });

  it("classes text/rtf as text, as the API does, and application/rtf as a document", () => {
    expect(typeClassOf("text/rtf")).toBe("text");
    expect(typeClassOf("application/rtf")).toBe("document");
  });

  it.each([
    "application/vnd.ms-word",
    "application/vnd.ms-excelfoo",
    "application/vnd.ms-fontobject",
    "application/vnd.ms-cab-compressed",
    "application/vnd.ms-outlook",
    "application/zip",
    "application/json",
    "font/woff2",
  ])("does not class %s as a document", (mediaType) => {
    expect(typeClassOf(mediaType)).toBeUndefined();
  });

  it("suggests the class a typed word names, else the classes of the files found", () => {
    expect(typeSuggestions("pres", [])).toEqual(["document"]);
    expect(typeSuggestions("holiday photo", [])).toEqual(["image"]);
    expect(typeSuggestions("harbour", ["application/pdf", "application/pdf", "image/png"])).toEqual(
      ["document", "image"],
    );
    expect(typeSuggestions("harbour", [])).toEqual([]);
  });
});
