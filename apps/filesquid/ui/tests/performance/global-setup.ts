/**
 * Before the performance run: record the machine the numbers come from (NFR-002 asks each
 * profile to declare its device, browser and conditions), so a result is never read without it.
 * Written to `tests/allure-results/performance/environment.json`; the browser build is added by
 * the control spec, which is the only one that has a browser at hand.
 */
import { mkdirSync, rmSync, writeFileSync } from "node:fs";
import { cpus, freemem, loadavg, platform, release, totalmem } from "node:os";
import { join } from "node:path";
import { RESULTS_DIR } from "./report";

/**
 * Write the environment record.
 *
 * @returns Resolves when the file is written.
 */
export default function globalSetup(): void {
  // Stale results would show as ghost measurements (test-cleanup rule), so start empty.
  rmSync(RESULTS_DIR, { recursive: true, force: true });
  mkdirSync(RESULTS_DIR, { recursive: true });
  const cores = cpus();
  const environment = {
    recorded: new Date().toISOString(),
    host: `${platform()} ${release()}`,
    cpu: cores[0]?.model ?? "unknown",
    cores: cores.length,
    memoryGiB: Math.round((totalmem() / 1024 ** 3) * 10) / 10,
    freeMemoryGiB: Math.round((freemem() / 1024 ** 3) * 10) / 10,
    loadBefore: loadavg().map((load) => Math.round(load * 100) / 100),
    node: process.version,
    note: "Headless Chromium in a container, loopback, fake host with no modelled latency; not the packaged macOS app.",
  };
  writeFileSync(join(RESULTS_DIR, "environment.json"), JSON.stringify(environment, null, 2));
}
