/**
 * Unit tests for the pure model of the Description tab (UI-F2.4; UI-S-022, UI-S-031 mockups 28
 * and 29, API-S-035, MD-S-002): statements become rows keyed by their CURIE with the prefix,
 * owner and extracted tones, and only asserted rows editable; `exif:` statements are technical
 * rows of the File info tab; the dialog's search over the file-scoped `/terms/` names; the
 * candidate groups in the API's add order; and the value, term-name and custom-key checks.
 */
import { describe, expect, it } from "vitest";
import {
  DCTERMS_FILE_TERMS,
  MAX_STATEMENT_ROWS,
  MAX_STATEMENT_VALUE_BYTES,
  MAX_TERM_MATCHES,
  TECHNICAL_PREFIXES,
  customKeyProblem,
  customNamespaces,
  dctermsDescription,
  descriptiveStatements,
  groupCandidates,
  matchingTerms,
  metadataCountText,
  statementRows,
  validTermName,
  valueProblem,
} from "../../src/files/metadata";
import labels from "../../src/theme/labels.en.json";
import { CUSTOM, RICO, metadataStatements } from "../support/metadata-fakes";

describe("descriptiveStatements", () => {
  it("keeps dcterms, schema and registered statements in record order and drops exif", () => {
    const kept = descriptiveStatements(metadataStatements());
    expect(TECHNICAL_PREFIXES).toEqual(["exif"]);
    expect(kept.map((s) => s.curie)).toEqual([
      "dcterms:title",
      "dcterms:creator",
      "dcterms:subject",
      "dcterms:description",
      "dcterms:created",
      "dcterms:rights",
      "schema:contentLocation",
      "schema:copyrightYear",
    ]);
  });

  it("is bounded", () => {
    const many = Array.from({ length: MAX_STATEMENT_ROWS + 5 }, (_, i) => ({
      curie: `harbour:k${i}`,
      prefix: "harbour",
      term: `k${i}`,
      value: "v",
      source: "asserted" as const,
    }));
    expect(descriptiveStatements(many)).toHaveLength(MAX_STATEMENT_ROWS);
  });
});

describe("statementRows", () => {
  it("keys each row by its CURIE, shows the prefix, and marks the tone and editability", () => {
    const rows = statementRows(metadataStatements());
    expect(rows[0]).toEqual({
      key: "dcterms:title",
      label: "dcterms:title",
      value: "Facade study, north pier",
      labelStyle: "owner",
      curie: "dcterms:title",
      prefix: "dcterms",
      source: "asserted",
      editable: true,
    });
    const created = rows.find((r) => r.curie === "dcterms:created");
    expect(created).toMatchObject({
      labelStyle: "extracted",
      source: "extracted",
      editable: false,
    });
    expect(rows.every((r) => r.label.includes(":"))).toBe(true);
    expect(rows.some((r) => r.prefix === "exif")).toBe(false);
    expect(rows).toHaveLength(8);
  });
});

describe("the file-scoped /terms/ names", () => {
  it("are the 55 ISO 15836-2 properties less the three accrual terms (MD-S-002)", () => {
    expect(DCTERMS_FILE_TERMS).toHaveLength(52);
    expect(DCTERMS_FILE_TERMS).toContain("title");
    expect(DCTERMS_FILE_TERMS).toContain("rightsHolder");
    expect(DCTERMS_FILE_TERMS).not.toContain("accrualMethod");
    expect(DCTERMS_FILE_TERMS).not.toContain("accrualPeriodicity");
    expect(DCTERMS_FILE_TERMS).not.toContain("accrualPolicy");
    expect([...DCTERMS_FILE_TERMS].sort()).toEqual([...new Set(DCTERMS_FILE_TERMS)].sort());
  });

  it("each has a description in the label table", () => {
    for (const term of DCTERMS_FILE_TERMS) {
      expect(dctermsDescription(term, labels), term).toBeTruthy();
    }
    expect(dctermsDescription("rights", labels)).toBe(labels.dctermsRights);
    expect(dctermsDescription("isPartOf", labels)).toBe(labels.dctermsIsPartOf);
    expect(dctermsDescription("nope", labels)).toBeUndefined();
  });
});

describe("matchingTerms", () => {
  it("lists names starting with the typed text first, then names containing it, ignoring case", () => {
    expect(matchingTerms("righ")).toEqual([
      "rights",
      "rightsHolder",
      "accessRights",
      "dateCopyrighted",
    ]);
    expect(matchingTerms("DATE")).toEqual([
      "date",
      "dateAccepted",
      "dateCopyrighted",
      "dateSubmitted",
    ]);
  });

  it("answers nothing for blank text and at most MAX_TERM_MATCHES names", () => {
    expect(matchingTerms("")).toEqual([]);
    expect(matchingTerms("  ")).toEqual([]);
    expect(matchingTerms("e").length).toBe(MAX_TERM_MATCHES);
    expect(MAX_TERM_MATCHES).toBe(8);
  });
});

describe("groupCandidates", () => {
  it("puts dcterms first, then the Schema.org suggestion, then the registered namespaces", () => {
    const groups = groupCandidates(
      "righ",
      {
        name: "righ",
        candidates: [
          { curie: "schema:righ", kind: "schema" },
          { curie: "rico:righ", kind: "extension" },
          { curie: "harbour:righ", kind: "custom" },
        ],
      },
      ["rights", "rightsHolder"],
    );
    expect(groups).toEqual({
      dcterms: ["dcterms:rights", "dcterms:rightsHolder"],
      schema: "schema:righ",
      registered: [
        { curie: "rico:righ", prefix: "rico", kind: "extension" },
        { curie: "harbour:righ", prefix: "harbour", kind: "custom" },
      ],
    });
  });

  it("uses only an answer for the typed name, and adds a dcterms candidate the local list lacks", () => {
    const stale = groupCandidates(
      "title",
      { name: "righ", candidates: [{ curie: "schema:righ", kind: "schema" }] },
      ["title"],
    );
    expect(stale).toEqual({ dcterms: ["dcterms:title"], schema: undefined, registered: [] });
    const extra = groupCandidates(
      "x",
      { name: "x", candidates: [{ curie: "dcterms:x", kind: "dcterms" }] },
      [],
    );
    expect(extra.dcterms).toEqual(["dcterms:x"]);
    expect(groupCandidates("", undefined, [])).toEqual({
      dcterms: [],
      schema: undefined,
      registered: [],
    });
  });
});

describe("the checks the dialog applies before sending", () => {
  it("validTermName follows the route: letters, digits, _, - and ., starting with a letter or _, 1 to 128 bytes", () => {
    expect(validTermName("title")).toBe(true);
    expect(validTermName("_x-1.y")).toBe(true);
    expect(validTermName("")).toBe(false);
    expect(validTermName("1abc")).toBe(false);
    expect(validTermName("a:b")).toBe(false);
    expect(validTermName("a b")).toBe(false);
    expect(validTermName("a".repeat(128))).toBe(true);
    expect(validTermName("a".repeat(129))).toBe(false);
  });

  it("valueProblem refuses a blank value and one over 65,536 UTF-8 bytes", () => {
    expect(MAX_STATEMENT_VALUE_BYTES).toBe(65_536);
    expect(valueProblem("")).toBe("empty");
    expect(valueProblem("   ")).toBe("empty");
    expect(valueProblem("ok")).toBeUndefined();
    expect(valueProblem("é".repeat(32_768))).toBeUndefined();
    expect(valueProblem("é".repeat(32_769))).toBe("too-long");
  });

  it("customKeyProblem wants prefix:name with a registered custom prefix and a valid name", () => {
    const registry = [RICO, CUSTOM];
    expect(customKeyProblem("harbour:berth", registry)).toBeUndefined();
    expect(customKeyProblem("harbour:1x", registry)).toBe("format");
    expect(customKeyProblem("berth", registry)).toBe("format");
    expect(customKeyProblem("rico:berth", registry)).toBe("prefix");
    expect(customKeyProblem("other:berth", registry)).toBe("prefix");
    expect(customNamespaces(registry)).toEqual([CUSTOM]);
  });
});

describe("metadataCountText", () => {
  it("counts the rows, the filtered rows, and words an empty record", () => {
    expect(metadataCountText(labels, 8, 8, "en")).toBe("Dublin Core and Schema.org · 8");
    expect(metadataCountText(labels, 2, 8, "en")).toBe("Dublin Core and Schema.org · 2 of 8");
    expect(metadataCountText(labels, 0, 0, "en")).toBe(labels.metadataCountEmpty);
  });
});
