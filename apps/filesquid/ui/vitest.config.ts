/**
 * Unit and UI tests (jsdom). Allure raw results go to tests/allure-results/vitest and JUnit
 * to tests/allure-results/vitest-junit.xml; `npm run report` builds the Allure report.
 */
import { defineConfig } from "vitest/config";

export default defineConfig({
  test: {
    environment: "jsdom",
    include: ["tests/unit/**/*.test.ts", "tests/ui/**/*.test.ts"],
    globalSetup: ["tests/support/pin-timezone.mjs"],
    setupFiles: ["allure-vitest/setup"],
    reporters: [
      "default",
      "junit",
      ["allure-vitest/reporter", { resultsDir: "tests/allure-results/vitest" }],
    ],
    outputFile: { junit: "tests/allure-results/vitest-junit.xml" },
    cache: false,
  },
});
