/**
 * The cross-platform test scripts in `tests/support/` (test-cleanup rule): the results cleaner
 * removes stale results, ignores what is already gone, refuses to run with no path and fails
 * when a removal fails; the e2e runner runs both Playwright suites in order and fails if either
 * fails (driven here with a stub CLI, so no browser starts).
 */
import { execFileSync, spawnSync } from "node:child_process";
import { existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { afterEach, beforeEach, describe, expect, it } from "vitest";

const CLEANER = resolve(process.cwd(), "tests/support/clean-results.mjs");
const RUNNER = resolve(process.cwd(), "tests/support/run-e2e.mjs");

/** A stand-in Playwright CLI: logs its arguments and exits with the status its env names. */
const STUB_CLI = `
import { appendFileSync } from "node:fs";
appendFileSync(process.env.STUB_LOG, process.argv.slice(2).join(" ") + "\\n");
const preview = process.argv.includes("playwright.preview.config.ts");
process.exitCode = Number(preview ? process.env.STUB_PREVIEW : process.env.STUB_SERVER);
`;

let dir = "";

beforeEach(() => {
  dir = mkdtempSync(join(tmpdir(), "ui-0.6-clean-"));
});

afterEach(() => {
  rmSync(dir, { recursive: true, force: true });
});

/** Run the cleaner with the given paths in the scratch directory. */
function clean(...paths: string[]): ReturnType<typeof spawnSync> {
  return spawnSync(process.execPath, [CLEANER, ...paths], { cwd: dir, encoding: "utf8" });
}

describe("clean-results.mjs", () => {
  it("removes a directory tree and a file", () => {
    mkdirSync(join(dir, "results", "playwright"), { recursive: true });
    writeFileSync(join(dir, "results", "playwright", "a-result.json"), "{}");
    writeFileSync(join(dir, "results", "junit.xml"), "<x/>");
    expect(clean("results/playwright", "results/junit.xml").status).toBe(0);
    expect(existsSync(join(dir, "results", "playwright"))).toBe(false);
    expect(existsSync(join(dir, "results", "junit.xml"))).toBe(false);
  });

  it("keeps what it was not asked to remove", () => {
    mkdirSync(join(dir, "results", "vitest"), { recursive: true });
    mkdirSync(join(dir, "results", "playwright"), { recursive: true });
    expect(clean("results/playwright").status).toBe(0);
    expect(existsSync(join(dir, "results", "vitest"))).toBe(true);
  });

  it("treats a missing path as already clean", () => {
    expect(clean("results/never-existed").status).toBe(0);
  });

  it("refuses to run with no path and says why", () => {
    const result = clean();
    expect(result.status).toBe(1);
    expect(String(result.stderr)).toContain("clean-results: no path given");
  });

  it("fails and names the path when a removal fails", () => {
    writeFileSync(join(dir, "afile"), "x");
    const result = clean("afile/child");
    expect(result.status).toBe(1);
    expect(String(result.stderr)).toContain("clean-results: afile/child:");
  });

  it("is a valid ES module that Node can parse", () => {
    expect(() => execFileSync(process.execPath, ["--check", CLEANER])).not.toThrow();
  });
});

/** Run the e2e runner against the stub CLI; returns its status and the logged invocations. */
function runStubbed(server: number, preview: number, ...extra: string[]) {
  const cli = join(dir, "stub-cli.mjs");
  const log = join(dir, "calls.txt");
  writeFileSync(cli, STUB_CLI);
  const env = {
    ...process.env,
    FILESQUID_PLAYWRIGHT_CLI: cli,
    STUB_LOG: log,
    STUB_SERVER: String(server),
    STUB_PREVIEW: String(preview),
  };
  const result = spawnSync(process.execPath, [RUNNER, ...extra], { cwd: dir, env });
  const calls = existsSync(log) ? readFileSync(log, "utf8").trim().split("\n") : [];
  return { status: result.status, calls };
}

describe("run-e2e.mjs", () => {
  it("runs the server-backed suite, then the preview suite, and passes when both pass", () => {
    const { status, calls } = runStubbed(0, 0);
    expect(status).toBe(0);
    expect(calls).toEqual(["test", "test -c playwright.preview.config.ts"]);
  });

  it("still runs the preview suite when the server-backed suite fails, and fails", () => {
    const { status, calls } = runStubbed(1, 0);
    expect(status).toBe(1);
    expect(calls).toHaveLength(2);
  });

  it("fails when only the preview suite fails", () => {
    expect(runStubbed(0, 1).status).toBe(1);
  });

  it("passes extra arguments to both suites", () => {
    const { calls } = runStubbed(0, 0, "--grep", "theme");
    expect(calls).toEqual([
      "test --grep theme",
      "test -c playwright.preview.config.ts --grep theme",
    ]);
  });
});
