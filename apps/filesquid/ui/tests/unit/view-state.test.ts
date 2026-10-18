// @vitest-environment node
/**
 * UI-F2.1a: a failed load is shown as one of three distinct states: offline when the browser
 * is offline, permission denied for an authorisation denial (API-S-004), error otherwise.
 */
import { describe, expect, it } from "vitest";
import { ApiError } from "../../src/api/client";
import { failureState } from "../../src/components/view-state";

describe("failureState", () => {
  it("is offline whenever the browser is offline", () => {
    expect(failureState(new TypeError("Failed to fetch"), false)).toEqual({ kind: "offline" });
    expect(failureState(new ApiError(500, "/api/v1/x"), false)).toEqual({ kind: "offline" });
  });

  it("is permission denied for an authorisation denial, with its reason", () => {
    const denied = new ApiError(403, "/api/v1/x", { reason: "not_permitted" });
    expect(failureState(denied, true)).toEqual({ kind: "denied", reason: "not_permitted" });
    const missing = new ApiError(401, "/api/v1/x", { reason: "missing_credentials" });
    expect(failureState(missing, true)).toEqual({ kind: "denied", reason: "missing_credentials" });
  });

  it("is an error for anything else", () => {
    expect(failureState(new ApiError(500, "/api/v1/x"), true)).toEqual({ kind: "error" });
    expect(failureState(new Error("boom"), true)).toEqual({ kind: "error" });
    expect(failureState("text", true)).toEqual({ kind: "error" });
  });
});
