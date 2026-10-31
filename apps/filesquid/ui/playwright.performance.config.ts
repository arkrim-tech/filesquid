/**
 * Browsing performance budgets (Task UI-F3.1, NFR-002, RC-002, ADR 007, ADR 017): the FileSquid
 * UI built by `vite build` and served by `vite preview` on this workspace's preview port, the
 * host faked in process, measured in this machine's Chromium at the mockups' 1440 x 900 window
 * at device scale factor 2. One worker and no retry: the specs time interactions, so nothing
 * else may share the browser, and a failed budget must fail, not pass on a second try. Tracing
 * is off because it perturbs the timings; the one traced run is recorded by its own test.
 * Results (JSON per measurement, JUnit, Allure) go to `tests/allure-results/`, cleared before
 * each run (`pretest:perf`).
 */
import { defineConfig, devices } from "@playwright/test";
import { DEVICE_SCALE_FACTOR, VIEWPORT } from "./tests/performance/budgets";

const PORT = 4173;

// The measured pages run with no host behind the preview server: strict-dom excuses the
// browser's line for the optional start-up reads only in this kind of suite.
process.env.FILESQUID_E2E_PREVIEW = "1";

export default defineConfig({
  testDir: "tests/performance",
  testMatch: "**/*.perf.spec.ts",
  outputDir: "tests/test-results",
  timeout: 900_000,
  workers: 1,
  fullyParallel: false,
  retries: 0,
  globalSetup: "./tests/performance/global-setup.ts",
  globalTeardown: "./tests/performance/global-teardown.ts",
  reporter: [
    ["list"],
    ["junit", { outputFile: "tests/allure-results/playwright-performance-junit.xml" }],
    ["allure-playwright", { resultsDir: "tests/allure-results/playwright" }],
  ],
  use: {
    baseURL: `http://127.0.0.1:${PORT}`,
    timezoneId: "UTC",
    trace: "off",
    viewport: VIEWPORT,
    deviceScaleFactor: DEVICE_SCALE_FACTOR,
  },
  projects: [
    {
      name: "chromium",
      use: {
        ...devices["Desktop Chrome"],
        viewport: VIEWPORT,
        deviceScaleFactor: DEVICE_SCALE_FACTOR,
      },
    },
  ],
  webServer: {
    command: `npm run build && npx vite preview --host 127.0.0.1 --port ${PORT} --strictPort`,
    url: `http://127.0.0.1:${PORT}/`,
    timeout: 300_000,
    reuseExistingServer: false,
  },
});
