/**
 * Production-artifact end-to-end tests (UI-S-013): `npm run build`, then `vite preview` serves
 * the static build on this workspace's preview port. No FileSquid server, no dev server.
 * Allure raw results go to tests/allure-results/playwright and JUnit beside them.
 */
import { defineConfig, devices } from "@playwright/test";

const PORT = 4174;

export default defineConfig({
  testDir: "tests/e2e/preview",
  outputDir: "tests/test-results",
  reporter: [
    ["list"],
    ["junit", { outputFile: "tests/allure-results/playwright-preview-junit.xml" }],
    ["allure-playwright", { resultsDir: "tests/allure-results/playwright" }],
  ],
  use: { baseURL: `http://127.0.0.1:${PORT}` },
  projects: [{ name: "chromium", use: { ...devices["Desktop Chrome"] } }],
  webServer: {
    command: `npm run build && npx vite preview --host 127.0.0.1 --port ${PORT} --strictPort`,
    url: `http://127.0.0.1:${PORT}/`,
    timeout: 300_000,
    reuseExistingServer: false,
  },
});
