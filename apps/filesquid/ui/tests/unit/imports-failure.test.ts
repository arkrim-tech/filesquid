/**
 * Unit tests for how the upload flow words a refusal (UI-1.2, UI-S-029, mockup 48): by the
 * answer's HTTP status and the step that failed, by a file's own error code, and by the UI's own
 * pre-send check, never from a sub-code the contract does not carry and never from the server's
 * free text (API-F2.21 adds the sub-codes later; UI-F2.17 refines this).
 */
import { describe, expect, it } from "vitest";
import {
  DestinationRequiredError,
  DestinationUnresolvedError,
  fileRefusal,
  importFailure,
} from "../../src/imports/failure";
import labels from "../../src/theme/labels.en.json";
import { jobItem, refusal } from "../support/import-fakes";

describe("a refused step (API-S-036, API-S-037)", () => {
  it("words the UI's own check of a missing destination as destination_required (mockup 48)", () => {
    expect(importFailure(new DestinationRequiredError(), "plan", labels, true)).toBe(
      "Choose a collection or a folder inside one. Files cannot go to a library's top level.",
    );
  });

  it("words a destination the host cannot name yet without inventing a code", () => {
    expect(importFailure(new DestinationUnresolvedError(), "plan", labels, true)).toBe(
      labels.importDestinationUnresolved,
    );
  });

  it("words a 404 of the plan request as the collection being gone (mockup 48)", () => {
    expect(importFailure(refusal(404, "not_found"), "plan", labels, true)).toBe(
      "The collection you chose is no longer in this library. Choose another and try again.",
    );
  });

  it("words a 404 of a plan route as the plan being gone", () => {
    for (const step of ["items", "start", "discard"] as const) {
      expect(importFailure(refusal(404, "not_found"), step, labels, true)).toBe(
        labels.importPlanGone,
      );
    }
  });

  it("words each other status and step", () => {
    expect(importFailure(refusal(400, "invalid_request"), "plan", labels, true)).toBe(
      labels.importInvalid,
    );
    expect(importFailure(refusal(409, "conflict"), "plan", labels, true)).toBe(
      labels.importDialogOpen,
    );
    expect(importFailure(refusal(409, "conflict"), "start", labels, true)).toBe(
      labels.importSourceChanged,
    );
    expect(importFailure(refusal(409, "conflict"), "job", labels, true)).toBe(labels.jobsRefused);
    expect(importFailure(refusal(503, "unavailable"), "plan", labels, true)).toBe(
      labels.importUnavailable,
    );
    expect(importFailure(refusal(503, "unavailable"), "job", labels, true)).toBe(
      labels.jobsUnavailable,
    );
    expect(importFailure(refusal(500, "internal"), "plan", labels, true)).toBe(labels.importFailed);
  });

  it("words each authorisation denial by its reason", () => {
    const words = {
      missing_credentials: labels.authMissingCredentials,
      invalid_credentials: labels.authInvalidCredentials,
      not_permitted: labels.authNotPermitted,
    } as const;
    for (const [reason, text] of Object.entries(words)) {
      const denied = refusal(403, "forbidden");
      Object.assign(denied, { reason });
      expect(importFailure(denied, "plan", labels, true)).toBe(text);
    }
  });

  it("words a failure with no answer as offline or as a failure, never with its message", () => {
    const broken = new Error("socket C:\\Users\\x\\secret closed");
    expect(importFailure(broken, "plan", labels, true)).toBe(labels.importFailed);
    expect(importFailure(broken, "plan", labels, false)).toBe(labels.importOffline);
  });
});

describe("a file's own refusal (mockup 48, API-S-036)", () => {
  it("words path_suffix_exhausted with the first and last name of the folder", () => {
    const file = jobItem("photo.jpg", { state: "failed", error_code: "path_suffix_exhausted" });
    expect(fileRefusal(file, labels)).toEqual({
      title: "Not uploaded: photo.jpg",
      reason:
        "Every name from photo-1.jpg to photo-9999.jpg is taken in this folder. Nothing was changed. Rename or remove one, then try again.",
    });
  });

  it("gives no card to a file that did not fail or whose code has no wording of its own", () => {
    expect(fileRefusal(jobItem("a.jpg", { state: "queued" }), labels)).toBeUndefined();
    expect(
      fileRefusal(jobItem("a.jpg", { state: "failed", error_code: "source_changed" }), labels),
    ).toBeUndefined();
    expect(fileRefusal(jobItem("a.jpg", { state: "failed" }), labels)).toBeUndefined();
  });
});
