/**
 * WebdriverIO configuration for the filesquid-desktop packaged-window tests (ADR 017, ADR 035).
 *
 * Drives the test-instrumented desktop binary (`cargo build -p filesquid-desktop --features
 * test-webdriver`) through the Tauri service's embedded WebDriver provider, so no external
 * driver is needed on Linux, macOS or Windows. On Linux without a display, run it under Xvfb.
 *
 * Environment:
 * - `FILESQUID_DESKTOP_BIN`: the binary to drive. Default: `<target>/debug/filesquid-desktop`,
 *   where `<target>` is `CARGO_TARGET_DIR` or `apps/filesquid/target`.
 * - `TAURI_WEBDRIVER_PORT`: the embedded WebDriver port (default 4445, on 127.0.0.1).
 */
import { mkdtempSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";

const here = path.dirname(fileURLToPath(import.meta.url));
const targetDir = process.env.CARGO_TARGET_DIR ?? path.resolve(here, "../../../../target");
const binaryName = process.platform === "win32" ? "filesquid-desktop.exe" : "filesquid-desktop";
const binary = process.env.FILESQUID_DESKTOP_BIN ?? path.join(targetDir, "debug", binaryName);
const port = Number(process.env.TAURI_WEBDRIVER_PORT ?? "4445");

// On Linux the app keeps its journal under XDG_DATA_HOME; give each run a fresh one so the
// tests never write into the developer's own FileSquid data.
process.env.XDG_DATA_HOME = mkdtempSync(path.join(tmpdir(), "filesquid-desktop-e2e-"));

export const config = {
  runner: "local",
  specs: ["./specs/**/*.e2e.mjs"],
  maxInstances: 1,
  services: [
    [
      "@wdio/tauri-service",
      { driverProvider: "embedded", appBinaryPath: binary, embeddedPort: port },
    ],
  ],
  capabilities: [{ browserName: "tauri", "tauri:options": { application: binary } }],
  logLevel: "warn",
  framework: "mocha",
  mochaOpts: { ui: "bdd", timeout: 180000 },
  reporters: ["spec", ["junit", { outputDir: path.resolve(here, "../allure-results/wdio") }]],
  waitforTimeout: 15000,
  connectionRetryTimeout: 120000,
  connectionRetryCount: 1,
};
