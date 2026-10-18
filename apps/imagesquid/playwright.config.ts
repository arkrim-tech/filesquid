/**
 * End-to-end tests against the BUILT ImageSquid distribution served by the real
 * `filesquid-server` (production artifacts, real FileSquid API). Port 4174.
 */
import { defineConfig, devices } from "@playwright/test";
import { ADMIN_TOKEN } from "./tests/e2e/admin-token";

const PORT = 4174;

export default defineConfig({
  testDir: "tests/e2e",
  outputDir: "tests/test-results",
  reporter: [["list"], ["junit", { outputFile: "tests/allure-results/playwright-junit.xml" }]],
  use: { baseURL: `http://127.0.0.1:${PORT}` },
  projects: [{ name: "chromium", use: { ...devices["Desktop Chrome"] } }],
  webServer: {
    command: "npm run build && cd ../filesquid && cargo run -q -p filesquid-server",
    url: `http://127.0.0.1:${PORT}/api/v1/health/live`,
    timeout: 300_000,
    reuseExistingServer: false,
    env: {
      FILESQUID_BIND: `127.0.0.1:${PORT}`,
      FILESQUID_ADMIN_TOKEN: ADMIN_TOKEN,
      FILESQUID_DATA_DIR: "target/e2e-imagesquid-data",
      FILESQUID_UI_DIR: "../imagesquid/dist",
    },
  },
});
