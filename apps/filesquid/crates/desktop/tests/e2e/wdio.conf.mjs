/**
 * WebdriverIO configuration for the filesquid-desktop packaged-window tests (ADR 017, ADR 035).
 *
 * Drives the test-instrumented desktop binary (`cargo build -p filesquid-desktop --features
 * test-webdriver`) through the Tauri service's embedded WebDriver provider, so no external
 * driver is needed on Linux, macOS or Windows. On Linux without a display, run it under Xvfb.
 *
 * Environment:
 * - `FILESQUID_DESKTOP_BIN`: the binary to drive. Default: `<target>/debug/filesquid-desktop`,
 *   where `<target>` is `CARGO_TARGET_DIR` (a relative value is resolved against the Cargo
 *   workspace `apps/filesquid`, as Cargo does when it is run from there) or
 *   `apps/filesquid/target`. A relative `FILESQUID_DESKTOP_BIN` is resolved against the
 *   current directory.
 * - `TAURI_WEBDRIVER_PORT`: the embedded WebDriver port (default 4445, on 127.0.0.1).
 *
 * The chosen binary is copied into a fresh temporary directory before the run, so a concurrent
 * Cargo build that replaces the shared output cannot change the binary under test.
 */
import { copyFileSync, mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";

const here = path.dirname(fileURLToPath(import.meta.url));
const workspace = path.resolve(here, "../../../..");
const targetDir = path.resolve(workspace, process.env.CARGO_TARGET_DIR ?? "target");
const binaryName = process.platform === "win32" ? "filesquid-desktop.exe" : "filesquid-desktop";
const built = process.env.FILESQUID_DESKTOP_BIN
  ? path.resolve(process.env.FILESQUID_DESKTOP_BIN)
  : path.join(targetDir, "debug", binaryName);
// The launcher and every worker load this file; the launcher makes the run directory and
// copies the binary once, and the workers (which inherit its environment) reuse both.
// Ownership: only the process that calls mkdtemp here owns the directory and may delete it.
// A path inherited by a worker or supplied by a caller in FILESQUID_DESKTOP_E2E_RUN is never
// removed, because this process cannot know what else lives there.
const ownsRun = !process.env.FILESQUID_DESKTOP_E2E_RUN;
const run = ownsRun
  ? mkdtempSync(path.join(tmpdir(), "filesquid-desktop-e2e-"))
  : process.env.FILESQUID_DESKTOP_E2E_RUN;
const binary = path.join(run, binaryName);
if (ownsRun) {
  copyFileSync(built, binary);
  process.env.FILESQUID_DESKTOP_E2E_RUN = run;
}
const port = Number(process.env.TAURI_WEBDRIVER_PORT ?? "4445");

// On Linux the app keeps its journal under XDG_DATA_HOME; give each run a fresh one so the
// tests never write into the developer's own FileSquid data.
process.env.XDG_DATA_HOME = path.join(run, "data");

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
  /**
   * Remove the run directory (binary copy and app data) once every worker has finished, but
   * only when this launcher created it (`ownsRun`); an inherited or caller-supplied path stays.
   */
  onComplete() {
    if (ownsRun) {
      rmSync(run, { recursive: true, force: true });
    }
  },
};
