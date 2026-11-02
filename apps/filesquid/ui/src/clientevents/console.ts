/**
 * Console capture for client events (UI-F2.16; UI-S-023, API-S-030): wraps `console.warn` and
 * `console.error` so each call also records one client event with its level, one fixed component
 * identifier and an error code. The message, its arguments, URLs, file names and stacks are read
 * only to pick the code and are never kept (they could name a file or a person).
 *
 * The capture never forwards its own output: the queue reports through {@link queueLog}, which
 * writes to the console as it was before the capture, and a call made while an event is being
 * recorded is not recorded again. The page keeps showing what it logged.
 */
import { ApiError } from "../api/client";
import type { ClientEventInput } from "./queue";

/** The one component every captured console call is attributed to. */
export const CONSOLE_COMPONENT = "console";

/** The start of the queue's own console lines. */
export const CAPTURE_LOG_PREFIX = "FileSquid client events:";

/** The console methods the capture wraps and reports through. */
export interface CaptureTarget {
  /** The warning method. */
  warn: (...args: unknown[]) => void;
  /** The error method. */
  error: (...args: unknown[]) => void;
}

/** Records one event; the queue's `record`. */
export type Recorder = (input: ClientEventInput) => string;

/** A code made of lower-case letters, digits and underscores, from an error's class name. */
function snake(name: string): string {
  const words = name.replace(/([a-z0-9])([A-Z])/g, "$1_$2").toLowerCase();
  const code = words.replace(/[^a-z0-9]+/g, "_").replace(/^_+|_+$/g, "");
  return code === "" ? "error" : code.slice(0, 64);
}

/**
 * The error code a console call is recorded under. An API refusal gives the contract code the
 * API sent; any other error gives the snake-case name of its class; anything else gives
 * `console_warn` or `console_error`. The message is never read. Pure.
 *
 * @param args What the page passed to the console.
 * @param level The console method used.
 * @returns The code.
 */
export function errorCodeOf(args: readonly unknown[], level: "warn" | "error"): string {
  for (const arg of args) {
    if (arg instanceof ApiError && arg.code) return arg.code;
  }
  for (const arg of args) {
    if (arg instanceof Error || arg instanceof DOMException) return snake(arg.name);
  }
  return `console_${level}`;
}

/**
 * The queue's way to report a problem it handled: the console as it was before the capture, so
 * the report is never forwarded as an event. Writes to the console.
 *
 * @param target The console to report to (the real one by default).
 * @returns A function taking a message and the problem.
 */
export function queueLog(
  target: CaptureTarget = console,
): (message: string, error?: unknown) => void {
  const warn = target.warn.bind(target);
  return (message, error) => warn(`${CAPTURE_LOG_PREFIX} ${message}`, error ?? "");
}

/**
 * Record every `console.warn` and `console.error` of the page as a client event. Side effect:
 * replaces the two methods of `target` until the returned function is called.
 *
 * @param record Records one event (`ClientEventQueue.record`).
 * @param target The console to wrap (the real one by default).
 * @returns A function that puts the original methods back.
 */
export function installConsoleCapture(
  record: Recorder,
  target: CaptureTarget = console,
): () => void {
  const original = { warn: target.warn, error: target.error };
  const report = original.warn.bind(target);
  let recording = false;
  const wrap = (level: "warn" | "error") =>
    function captured(...args: unknown[]): void {
      original[level].apply(target, args);
      if (recording) return;
      recording = true;
      try {
        record({ level, component: CONSOLE_COMPONENT, error_code: errorCodeOf(args, level) });
      } catch (error) {
        report(`${CAPTURE_LOG_PREFIX} could not record a console event`, error);
      } finally {
        recording = false;
      }
    };
  target.warn = wrap("warn");
  target.error = wrap("error");
  return () => {
    target.warn = original.warn;
    target.error = original.error;
  };
}
