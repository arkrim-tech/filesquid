/**
 * Run both Playwright suites one after the other and fail if either fails: the server-backed
 * suite (`playwright.config.ts`), then the static `vite preview` suite
 * (`playwright.preview.config.ts`). They share a port, so they never run together. Unlike a
 * `&&` chain, a failure in the first suite does not hide the result of the second.
 *
 * Usage: `node tests/support/run-e2e.mjs [extra playwright arguments]`
 * Exits with 0 only when both suites pass.
 */
import { spawnSync } from "node:child_process";
import { createRequire } from "node:module";

/**
 * The Playwright CLI entry point, run through Node so it needs no shell on any OS.
 * `FILESQUID_PLAYWRIGHT_CLI` replaces it, so a unit test can drive this runner with a stub.
 */
const cli =
  process.env.FILESQUID_PLAYWRIGHT_CLI ??
  createRequire(import.meta.url).resolve("@playwright/test/cli");

/** Config files in run order; `undefined` selects the default config. */
const CONFIGS = [undefined, "playwright.preview.config.ts"];

/**
 * Run one Playwright suite.
 *
 * @param {string | undefined} config - Config file, or undefined for the default.
 * @param {string[]} extra - Extra Playwright arguments.
 * @returns {number} The suite's exit status (1 when the process could not start).
 */
function runSuite(config, extra) {
  const args = [cli, "test", ...(config ? ["-c", config] : []), ...extra];
  const result = spawnSync(process.execPath, args, { stdio: "inherit" });
  if (result.error) {
    console.error(`run-e2e: could not start Playwright: ${result.error.message}`);
    return 1;
  }
  return result.status ?? 1;
}

/**
 * Entry point: run every suite, then report the combined status.
 *
 * @param {string[]} extra - Extra Playwright arguments.
 * @returns {number} 0 when every suite passed, 1 otherwise.
 */
function main(extra) {
  const statuses = CONFIGS.map((config) => runSuite(config, extra));
  return statuses.every((status) => status === 0) ? 0 : 1;
}

process.exitCode = main(process.argv.slice(2));
