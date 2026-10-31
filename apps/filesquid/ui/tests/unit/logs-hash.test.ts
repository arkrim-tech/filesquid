/**
 * Unit tests for the Logs view's address (UI-F2.15; UI-S-020, UI-S-021): the Provenance tab's
 * View in Logs link opens `#/logs` filtered to a file (`file`, a UUID) or an operation
 * (`operation_id`), the names of the journal route's filters (API-S-031). UI-F2.16 parses the
 * same address with `parseLogsHash`.
 */
import { describe, expect, it } from "vitest";
import { LOGS_ROUTE, logsHash, parseLogsHash } from "../../src/app/routes";

describe("logsHash and parseLogsHash", () => {
  it("is the bare Logs route without a filter", () => {
    expect(logsHash({})).toBe(LOGS_ROUTE);
    expect(parseLogsHash(LOGS_ROUTE)).toEqual({});
  });

  it("filters to a file or an operation with the journal route's filter names", () => {
    expect(logsHash({ file: "7f3c9a1e-0b2c-4d5e-8f90-a1b2c341d2b8" })).toBe(
      "#/logs?file=7f3c9a1e-0b2c-4d5e-8f90-a1b2c341d2b8",
    );
    expect(logsHash({ operation_id: "op:1/a b" })).toBe("#/logs?operation_id=op%3A1%2Fa+b");
  });

  it("round-trips the filters", () => {
    const filter = { file: "f1", operation_id: "op-1" };
    expect(parseLogsHash(logsHash(filter))).toEqual(filter);
    expect(parseLogsHash(logsHash({ operation_id: "op:1/a b" }))).toEqual({
      operation_id: "op:1/a b",
    });
  });

  it("returns undefined for another route, and ignores a filter it does not know", () => {
    expect(parseLogsHash("#/files")).toBeUndefined();
    expect(parseLogsHash("#/logsx?file=a")).toBeUndefined();
    expect(parseLogsHash("#/logs?other=1&file=a")).toEqual({ file: "a" });
  });

  it("drops an empty value", () => {
    expect(parseLogsHash("#/logs?file=")).toEqual({});
    expect(parseLogsHash("#/logs?file=&operation_id=op-1")).toEqual({ operation_id: "op-1" });
  });
});
