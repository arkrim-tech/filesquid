/**
 * After the performance run: print every measurement as one table (sample size, warm-up, p50,
 * p95, p99, maximum, budget, verdict), with the environment and the load average at the end,
 * and write the same as Markdown to `tests/allure-results/performance/summary.md`. The
 * Walkthrough copies this output.
 */
import { readdirSync, readFileSync, writeFileSync } from "node:fs";
import { loadavg } from "node:os";
import { join } from "node:path";
import { RESULTS_DIR, type Measurement } from "./report";

/** A number with one decimal. */
function ms(value: number | undefined): string {
  return value === undefined ? "" : String(Math.round(value * 10) / 10);
}

/** The table row of one measurement. */
function row(record: Measurement): string {
  const s = record.summary;
  const f = record.frames;
  const verdict = !record.gated ? "reported" : record.failures.length === 0 ? "pass" : "FAIL";
  const n = s?.n ?? f?.frames ?? record.raw.length;
  const cells = s
    ? [ms(s.p50), ms(s.p90), ms(s.p95), ms(s.p99), ms(s.max)]
    : f
      ? [ms(f.p50), "", ms(f.p95), ms(f.p99), ms(f.max)]
      : ["", "", "", "", ""];
  return `| ${record.id} | ${n} | ${record.warmup} | ${cells.join(" | ")} | ${valueOf(record)} | ${record.budget} | ${verdict} |`;
}

/** The measured scalar of a single-value record (DOM rows, a height, heap growth), else empty. */
function valueOf(record: Measurement): string {
  const scalar = record.notes["value"];
  return record.summary || record.frames || scalar === undefined ? "" : String(scalar);
}

/**
 * Print and write the summary.
 *
 * @returns Resolves when the summary is written.
 */
export default function globalTeardown(): void {
  const files = readdirSync(RESULTS_DIR).filter(
    (f) => f.endsWith(".json") && f !== "environment.json",
  );
  const records = files.map(
    (f) => JSON.parse(readFileSync(join(RESULTS_DIR, f), "utf8")) as Measurement,
  );
  records.sort((a, b) => a.id.localeCompare(b.id));
  const environment = JSON.parse(
    readFileSync(join(RESULTS_DIR, "environment.json"), "utf8"),
  ) as Record<string, unknown>;
  environment["loadAfter"] = loadavg().map((load) => Math.round(load * 100) / 100);
  const head = [
    "| measurement | n | warm-up | p50 | p90 | p95 | p99 | max | value | budget | verdict |",
    "|---|---|---|---|---|---|---|---|---|---|---|",
  ];
  const lines = [
    "Performance summary (milliseconds; frame rows are frame intervals)",
    "",
    "```json",
    JSON.stringify(environment, null, 2),
    "```",
    "",
    ...head,
    ...records.map(row),
  ];
  const text = lines.join("\n");
  writeFileSync(join(RESULTS_DIR, "summary.md"), text);
  console.log(`\n${text}\n`);
}
