/**
 * Test reporting contract (UI-S-013, ADR 017, test-cleanup rule): every run starts from a
 * clean `tests/allure-results/`, each runner writes Allure raw results and JUnit there, an
 * Allure report can be built from them, and the production Playwright proofs run against the
 * built artifact on this workspace's fixed preview port. Assertions match code, not the
 * header comments of the files they read.
 */
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";

/** The preview port of this package (never changes: port-conflicts rule). */
const PREVIEW_PORT = 4173;

/** Read a package-relative text file with its comments removed (block and line). */
function code(file: string): string {
  return read(file)
    .replace(/\/\*[\s\S]*?\*\//g, "")
    .replace(/^\s*\/\/.*$/gm, "");
}

/** Read a package-relative text file. */
function read(file: string): string {
  return readFileSync(resolve(process.cwd(), file), "utf8");
}

const pkg = JSON.parse(read("package.json")) as {
  scripts: Record<string, string>;
  devDependencies: Record<string, string>;
};

describe("clean results before each run", () => {
  it("clears the whole results directory before the unit run, on every OS", () => {
    expect(pkg.scripts["pretest"]).toBe(
      "node tests/support/clean-results.mjs tests/allure-results",
    );
  });

  it("clears only the Playwright results before an end-to-end run", () => {
    const script = pkg.scripts["pretest:e2e"] ?? "";
    expect(script).toMatch(/^node tests\/support\/clean-results\.mjs /);
    expect(script).toContain("tests/allure-results/playwright ");
    expect(script).toContain("tests/allure-results/playwright-junit.xml");
    expect(script).toContain("tests/allure-results/playwright-preview-junit.xml");
    expect(script).toContain("tests/allure-results/playwright-performance-junit.xml");
    expect(script).toContain("tests/allure-results/performance");
    expect(script).not.toMatch(/tests\/allure-results(\s|$)/);
  });

  it("uses no POSIX-only command in any script", () => {
    for (const [name, script] of Object.entries(pkg.scripts)) {
      expect(script, name).not.toMatch(/\brm\b/);
    }
  });
});

describe("Allure and JUnit output", () => {
  it("declares the Allure integrations and CLI as dev dependencies", () => {
    for (const name of ["allure", "allure-playwright", "allure-vitest"]) {
      expect(pkg.devDependencies[name], name).toBeTruthy();
    }
  });

  it("writes Vitest Allure results beside the JUnit file", () => {
    const config = code("vitest.config.ts");
    expect(config).toContain('"allure-vitest/reporter"');
    expect(config).toContain('resultsDir: "tests/allure-results/vitest"');
    expect(config).toContain('outputFile: { junit: "tests/allure-results/vitest-junit.xml" }');
  });

  it.each([
    "playwright.config.ts",
    "playwright.preview.config.ts",
    "playwright.performance.config.ts",
  ])("writes Playwright Allure results and JUnit from %s", (file) => {
    const config = code(file);
    expect(config).toContain('"allure-playwright"');
    expect(config).toContain('resultsDir: "tests/allure-results/playwright"');
    expect(config).toMatch(/outputFile: "tests\/allure-results\/playwright[a-z-]*-junit\.xml"/);
  });

  it("generates the Allure report into an untracked directory", () => {
    expect(pkg.scripts["report"]).toContain("allure generate");
    expect(pkg.scripts["report"]).toContain("tests/allure-report");
    expect(read(".gitignore")).toContain("tests/allure-report");
  });

  it("keeps the generated report out of Prettier and ESLint", () => {
    expect(read(".prettierignore")).toContain("tests/allure-report");
    expect(read("eslint.config.js")).toContain("tests/allure-report");
  });
});

describe("production artifact suite", () => {
  it("serves the built artifact with vite preview on the fixed port", () => {
    const config = code("playwright.preview.config.ts");
    expect(config).toMatch(/command: `npm run build && npx vite preview [^`]*--strictPort`/);
    expect(config).toContain("--port ${PORT}");
    expect(config).toMatch(new RegExp(`const PORT = ${PREVIEW_PORT};`));
    expect(config).not.toMatch(/npm run dev|npx vite(\s+dev)?\s*[`"']/);
  });

  it("keeps the preview specs out of the server-backed run and runs both in turn", () => {
    expect(code("playwright.config.ts")).toContain('testIgnore: "**/preview/**"');
    expect(pkg.scripts["test:e2e"]).toBe("node tests/support/run-e2e.mjs");
  });

  it("runs the performance suite on the same built artifact, with the budgets as assertions", () => {
    const config = code("playwright.performance.config.ts");
    expect(config).toMatch(/command: `npm run build && npx vite preview [^`]*--strictPort`/);
    expect(config).toMatch(new RegExp(`const PORT = ${PREVIEW_PORT};`));
    expect(config).toContain('testDir: "tests/performance"');
    expect(config).toContain("workers: 1");
    expect(config).toContain("retries: 0");
    expect(config).toContain('trace: "off"');
    expect(pkg.scripts["test:perf"]).toBe("playwright test -c playwright.performance.config.ts");
    expect(pkg.scripts["pretest:perf"]).toContain("tests/allure-results/performance");
    expect(pkg.scripts["pretest:perf"]).toContain(
      "tests/allure-results/playwright-performance-junit.xml",
    );
    // The performance run writes its Allure results into the shared Playwright directory, so a
    // standalone run clears it too, or it would report the results of an earlier suite.
    expect(pkg.scripts["pretest:perf"]).toMatch(/tests\/allure-results\/playwright(\s|$)/);
  });

  it("keeps the performance specs out of the other two Playwright runs", () => {
    expect(code("playwright.config.ts")).toContain('testDir: "tests/e2e"');
    expect(code("playwright.preview.config.ts")).toContain('testDir: "tests/e2e/preview"');
  });

  it("keeps the preview port in the Vite config", () => {
    expect(read("vite.config.ts")).toMatch(new RegExp(`preview:[^}]*port: ${PREVIEW_PORT}\\b`));
  });
});
