/**
 * Unit tests for the pure command and byte-range helpers (UI-S-012, CORE-S-005): envelope
 * building, outcome validation, durability and RFC 9110 ranges.
 */
import { describe, expect, it, vi } from "vitest";
import {
  commandEnvelope,
  commandOutcome,
  conflictOutcome,
  isCommitted,
  newOperationId,
  parseContentRange,
  rangeHeader,
} from "../../src/api/commands";

/** RFC 9562 version 4 UUID, lower case. */
const UUID_V4 = /^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/;

describe("commandEnvelope", () => {
  it("builds a schema version 1 envelope with the caller's scope and revision", () => {
    const envelope = commandEnvelope({
      scope: "library:abc",
      expectedRevision: "v3",
      operationId: "op-1",
    });
    expect(envelope).toEqual({
      schema_version: 1,
      operation_id: "op-1",
      expected_revision: "v3",
      scope: "library:abc",
    });
  });

  it("generates a fresh operation id when none is given", () => {
    const first = commandEnvelope({ scope: "library:abc", expectedRevision: "v3" });
    const second = commandEnvelope({ scope: "library:abc", expectedRevision: "v3" });
    expect(first.operation_id).not.toBe("");
    expect(first.operation_id).not.toBe(second.operation_id);
  });

  it("keeps a supplied operation id, so a retry is the same idempotent operation", () => {
    const id = newOperationId();
    const retry = commandEnvelope({ scope: "s", expectedRevision: "r", operationId: id });
    expect(retry.operation_id).toBe(id);
  });

  it.each([
    ["scope", { scope: "", expectedRevision: "r" }],
    ["expected_revision", { scope: "s", expectedRevision: "" }],
    ["operation_id", { scope: "s", expectedRevision: "r", operationId: "" }],
  ])("refuses an empty %s (contract minLength 1)", (field, input) => {
    expect(() => commandEnvelope(input)).toThrow(new RangeError(`${field} must not be empty`));
  });
});

describe("newOperationId", () => {
  it("returns a UUID from the platform's secure random source", () => {
    expect(newOperationId()).toMatch(UUID_V4);
  });

  it("works without crypto.randomUUID (plain HTTP is not a secure context)", () => {
    const fill = (bytes: Uint8Array) => bytes.fill(0xff);
    vi.stubGlobal("crypto", { getRandomValues: fill });
    try {
      expect(newOperationId()).toBe("ffffffff-ffff-4fff-bfff-ffffffffffff");
    } finally {
      vi.unstubAllGlobals();
    }
  });
});

describe("commandOutcome", () => {
  it.each(["committed", "pending", "conflict"] as const)(
    "accepts kind %s with a null error",
    (kind) => {
      const value = { operation_id: "op-1", kind, error: null };
      expect(commandOutcome(value, "op-1")).toEqual(value);
    },
  );

  it("accepts kind error with a PortError", () => {
    const value = {
      operation_id: "op-1",
      kind: "error",
      error: { code: "disk_full", retryable: true },
    };
    expect(commandOutcome(value, "op-1")).toEqual(value);
  });

  it("keeps the result a file write route answers beside the outcome (API-F2.13)", () => {
    const result = { library_id: "l", object_id: "o", version: "v2", created: "t" };
    const value = { operation_id: "op-1", kind: "committed", error: null, result, extra: 1 };
    expect(commandOutcome(value, "op-1")).toEqual({
      operation_id: "op-1",
      kind: "committed",
      error: null,
      result,
    });
  });

  it("drops fields the contract does not define", () => {
    const value = { operation_id: "op-1", kind: "committed", error: null, extra: "x" };
    expect(commandOutcome(value, "op-1")).toEqual({
      operation_id: "op-1",
      kind: "committed",
      error: null,
    });
  });

  it.each([
    ["a non-object", "committed"],
    ["an unknown kind", { operation_id: "op-1", kind: "done", error: null }],
    ["kind error without a PortError", { operation_id: "op-1", kind: "error", error: null }],
    [
      "a PortError with an empty code",
      { operation_id: "op-1", kind: "error", error: { code: "", retryable: false } },
    ],
    [
      "a PortError without retryable",
      { operation_id: "op-1", kind: "error", error: { code: "x" } },
    ],
    [
      "kind committed with an error",
      { operation_id: "op-1", kind: "committed", error: { code: "x", retryable: false } },
    ],
  ])("rejects %s", (_name, value) => {
    expect(() => commandOutcome(value, "op-1")).toThrow(TypeError);
  });

  it("rejects an empty operation id, even when both sides are empty", () => {
    const value = { operation_id: "", kind: "committed", error: null };
    expect(() => commandOutcome(value, "")).toThrow(
      new TypeError("command answer has no operation id"),
    );
  });

  it("rejects an outcome for another operation id", () => {
    const value = { operation_id: "op-2", kind: "committed", error: null };
    expect(() => commandOutcome(value, "op-1")).toThrow(
      new TypeError("command outcome is for another operation"),
    );
  });
});

describe("conflictOutcome and isCommitted", () => {
  it("builds a conflict outcome for the sent operation id", () => {
    expect(conflictOutcome("op-1")).toEqual({
      operation_id: "op-1",
      kind: "conflict",
      error: null,
    });
  });

  it("refuses to build a conflict outcome for an empty operation id", () => {
    expect(() => conflictOutcome("")).toThrow(new RangeError("operation_id must not be empty"));
  });

  it("treats only committed as durable completion evidence", () => {
    const outcome = (kind: "committed" | "pending" | "conflict") => ({
      operation_id: "op",
      kind,
      error: null,
    });
    expect(isCommitted(outcome("committed"))).toBe(true);
    expect(isCommitted(outcome("pending"))).toBe(false);
    expect(isCommitted(outcome("conflict"))).toBe(false);
    expect(
      isCommitted({ operation_id: "op", kind: "error", error: { code: "x", retryable: false } }),
    ).toBe(false);
  });
});

describe("rangeHeader", () => {
  it("writes an inclusive byte range", () => {
    expect(rangeHeader({ start: 0, end: 1023 })).toBe("bytes=0-1023");
  });
  it("writes an open-ended range", () => {
    expect(rangeHeader({ start: 4096 })).toBe("bytes=4096-");
  });
  it("accepts a one-byte range (end equals start)", () => {
    expect(rangeHeader({ start: 7, end: 7 })).toBe("bytes=7-7");
  });
  it.each([
    [{ start: -1 }],
    [{ start: 1.5 }],
    [{ start: 10, end: 9 }],
    [{ start: 0, end: 2.5 }],
    [{ start: Number.NaN }],
    [{ start: Number.MAX_SAFE_INTEGER + 1 }],
  ])("refuses %j", (range) => {
    expect(() => rangeHeader(range)).toThrow(RangeError);
  });
});

describe("parseContentRange", () => {
  it("reads start, end and complete length", () => {
    expect(parseContentRange("bytes 0-1023/104857600")).toEqual({
      start: 0,
      end: 1023,
      size: 104857600,
    });
  });
  it("leaves size undefined when the length is unknown", () => {
    expect(parseContentRange("bytes 10-19/*")).toEqual({ start: 10, end: 19, size: undefined });
  });
  it("accepts a last byte one below the complete length", () => {
    expect(parseContentRange("bytes 0-9/10")).toEqual({ start: 0, end: 9, size: 10 });
  });
  it.each([
    [null],
    [""],
    ["bytes */1000"],
    ["items 0-1/2"],
    ["bytes 9-1/20"],
    ["bytes 0-9/9"],
    ["bytes 0-9/5"],
    ["bytes 0-9007199254740992/*"],
    ["bytes 0-1/9007199254740993"],
  ])("returns undefined for %j", (value) => {
    expect(parseContentRange(value)).toBeUndefined();
  });
});
