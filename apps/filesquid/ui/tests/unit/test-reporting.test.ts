/**
 * Test reporting contract (UI-S-013, ADR 017, test-cleanup rule): every run starts from a
 * clean `tests/allure-results/`, each runner writes Allure raw results and JUnit there, an
 * Allure report can be built from them, and the production Playwright proofs run against the
 * built artifact on this workspace's fixed preview port.
 */
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";

/** The preview port of this package (never changes: port-conflicts rule). */
const PREVIEW_PORT = 4173;

/** Read a package-relative text file. */
function read(file: string): string {
  return readFileSync(resolve(process.cwd(), file), "utf8");
}

const pkg = JSON.parse(read("package.json")) as {
  scripts: Record<string, string>;
  devDependencies: Record<string, string>;
};

describe("clean results before each run", () => {
  it("clears the whole results directory before the unit run", () => {
    expect(pkg.scripts["pretest"]).toBe("rm -rf tests/allure-results");
  });

  it("clears only the Playwright results before an end-to-end run", () => {
    expect(pkg.scripts["pretest:e2e"]).toContain("tests/allure-results/playwright");
    expect(pkg.scripts["pretest:e2e"]).toContain("playwright-junit.xml");
    expect(pkg.scripts["pretest:e2e"]).not.toBe("rm -rf tests/allure-results");
  });
});

describe("Allure and JUnit output", () => {
  it("declares the Allure integrations and CLI as dev dependencies", () => {
    for (const name of ["allure", "allure-playwright", "allure-vitest"]) {
      expect(pkg.devDependencies[name], name).toBeTruthy();
    }
  });

  it("writes Vitest Allure results beside the JUnit file", () => {
    const config = read("vitest.config.ts");
    expect(config).toContain("allure-vitest/reporter");
    expect(config).toContain("tests/allure-results/vitest");
    expect(config).toContain("tests/allure-results/vitest-junit.xml");
  });

  it.each(["playwright.config.ts", "playwright.preview.config.ts"])(
    "writes Playwright Allure results and JUnit from %s",
    (file) => {
      const config = read(file);
      expect(config).toContain("allure-playwright");
      expect(config).toContain("tests/allure-results/playwright");
      expect(config).toContain("-junit.xml");
    },
  );

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
    const config = read("playwright.preview.config.ts");
    expect(config).toContain("npm run build");
    expect(config).toContain("vite preview");
    expect(config).toContain("--strictPort");
    expect(config).toContain(`const PORT = ${PREVIEW_PORT}`);
    expect(config).not.toMatch(/vite(\s+dev)?\s*["'`]|npm run dev/);
  });

  it("keeps the preview specs out of the server-backed run and runs both in turn", () => {
    expect(read("playwright.config.ts")).toContain('testIgnore: "**/preview/**"');
    const script = pkg.scripts["test:e2e"] ?? "";
    expect(script).toContain("playwright test");
    expect(script).toContain("-c playwright.preview.config.ts");
  });

  it("keeps the preview port in the Vite config", () => {
    expect(read("vite.config.ts")).toMatch(new RegExp(`preview:[^}]*port: ${PREVIEW_PORT}`));
  });
});
