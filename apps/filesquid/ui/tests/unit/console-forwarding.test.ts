/**
 * Unit tests for the console capture (UI-F2.16, UI-S-023): the page's `console.warn` and
 * `console.error` become client events that carry a level, one fixed component and an error code,
 * never the message, a URL, a file name or a stack; the capture ignores its own output and
 * restores the console when it is removed.
 */
import { afterEach, describe, expect, it, vi } from "vitest";
import { ApiError } from "../../src/api/client";
import type { ClientEventInput } from "../../src/clientevents/queue";
import {
  CAPTURE_LOG_PREFIX,
  CONSOLE_COMPONENT,
  errorCodeOf,
  installConsoleCapture,
  queueLog,
} from "../../src/clientevents/console";

/** A console stand-in whose methods are spies. */
function fakeConsole() {
  return { warn: vi.fn(), error: vi.fn(), log: vi.fn() };
}

/** A recorder that answers an event ID. */
function recorder() {
  return vi.fn<(input: ClientEventInput) => string>(() => "id");
}

afterEach(() => vi.restoreAllMocks());

describe("what a console call becomes", () => {
  it("is one event with the level, the fixed component and an error code", () => {
    const target = fakeConsole();
    const record = recorder();
    installConsoleCapture(record, target);
    target.warn("could not read /Users/me/IMG_4102.jpg from https://x.example/a?token=1");
    expect(record).toHaveBeenCalledTimes(1);
    expect(record).toHaveBeenCalledWith({
      level: "warn",
      component: CONSOLE_COMPONENT,
      error_code: "console_warn",
    });
  });

  it("never carries the message, a URL, a file name or a stack", () => {
    const target = fakeConsole();
    const record = recorder();
    installConsoleCapture(record, target);
    const failure = new TypeError("Cannot read IMG_4102.jpg at https://x.example/");
    target.error("FileSquid failed", failure, { path: "/home/me/secret.txt" });
    const sent = JSON.stringify(record.mock.calls);
    expect(sent).not.toContain("IMG_4102");
    expect(sent).not.toContain("x.example");
    expect(sent).not.toContain("secret.txt");
    expect(sent).not.toContain("FileSquid failed");
    expect(sent).not.toContain("stack");
    expect(Object.keys(record.mock.calls[0]?.[0] ?? {}).sort()).toEqual([
      "component",
      "error_code",
      "level",
    ]);
  });

  it("names the error code of an API refusal, and the class of any other error", () => {
    expect(errorCodeOf([new ApiError(429, "/x", { code: "rate_limited" })], "warn")).toBe(
      "rate_limited",
    );
    expect(errorCodeOf([new TypeError("x")], "error")).toBe("type_error");
    expect(errorCodeOf([new DOMException("x", "QuotaExceededError")], "error")).toBe(
      "quota_exceeded_error",
    );
    expect(errorCodeOf(["text", 3], "error")).toBe("console_error");
  });

  it("forwards warnings and errors only", () => {
    const target = fakeConsole();
    const record = recorder();
    installConsoleCapture(record, target);
    target.log("a");
    target.warn("b");
    target.error("c");
    expect(record.mock.calls.map((call) => call[0].level)).toEqual(["warn", "error"]);
  });
});

describe("the console itself", () => {
  it("still shows what was logged, once, as before", () => {
    const target = fakeConsole();
    const warn = target.warn;
    installConsoleCapture(() => "id", target);
    target.warn("shown", 1);
    expect(warn).toHaveBeenCalledWith("shown", 1);
    expect(warn).toHaveBeenCalledTimes(1);
  });

  it("is restored when the capture is removed", () => {
    const target = fakeConsole();
    const warn = target.warn;
    const stop = installConsoleCapture(() => "id", target);
    expect(target.warn).not.toBe(warn);
    stop();
    expect(target.warn).toBe(warn);
  });

  it("is not recorded again when the recording itself logs", () => {
    const target = fakeConsole();
    const record = vi.fn<(input: ClientEventInput) => string>(() => {
      target.warn("logged while recording");
      return "id";
    });
    installConsoleCapture(record, target);
    target.warn("first");
    expect(record).toHaveBeenCalledTimes(1);
  });

  it("ignores the queue's own reports, which go to the console unforwarded", () => {
    const target = fakeConsole();
    const original = target.warn;
    const record = recorder();
    const log = queueLog(target);
    installConsoleCapture(record, target);
    log("could not persist", new Error("quota"));
    expect(record).not.toHaveBeenCalled();
    expect(original).toHaveBeenCalledWith(
      `${CAPTURE_LOG_PREFIX} could not persist`,
      expect.anything(),
    );
  });

  it("does not stop the page when recording throws, and says so on the unforwarded console", () => {
    const target = fakeConsole();
    const original = target.warn;
    const record = vi.fn<(input: ClientEventInput) => string>(() => {
      throw new Error("queue closed");
    });
    installConsoleCapture(record, target);
    expect(() => target.warn("x")).not.toThrow();
    expect(original).toHaveBeenCalledTimes(2);
  });
});
