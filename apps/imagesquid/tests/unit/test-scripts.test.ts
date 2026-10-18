/**
 * The cross-platform test scripts in `tests/support/` (test-cleanup rule): the results cleaner
 * removes stale results, ignores what is already gone and refuses to run with no path.
 */
import { execFileSync, spawnSync } from "node:child_process";
import { existsSync, mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { afterEach, beforeEach, describe, expect, it } from "vitest";

const CLEANER = resolve(process.cwd(), "tests/support/clean-results.mjs");

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

  it("is a valid ES module that Node can parse", () => {
    expect(() => execFileSync(process.execPath, ["--check", CLEANER])).not.toThrow();
  });
});
