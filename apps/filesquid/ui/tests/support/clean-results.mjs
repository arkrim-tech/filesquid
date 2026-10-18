/**
 * Remove stale test results before a run (test-cleanup rule). Works on every OS, unlike
 * `rm -rf`, which npm's default shell on Windows does not have.
 *
 * Usage: `node tests/support/clean-results.mjs <path> [<path> ...]`
 * Every path is removed recursively; a path that does not exist is not an error.
 * Exits with status 1 when no path is given or a removal fails, so a broken cleanup stops
 * the run instead of leaving ghost results behind.
 */
import { rmSync } from "node:fs";

/**
 * Remove each path, collecting the failures.
 *
 * @param {string[]} paths - Paths relative to the package directory.
 * @returns {string[]} One message per path that could not be removed.
 */
export function removeAll(paths) {
  const failures = [];
  for (const path of paths) {
    try {
      rmSync(path, { recursive: true, force: true });
    } catch (error) {
      failures.push(`${path}: ${error instanceof Error ? error.message : String(error)}`);
    }
  }
  return failures;
}

/**
 * Entry point: clean the paths named on the command line.
 *
 * @param {string[]} argv - Command-line paths.
 * @returns {number} Process exit status.
 */
function main(argv) {
  if (argv.length === 0) {
    console.error("clean-results: no path given");
    return 1;
  }
  const failures = removeAll(argv);
  for (const failure of failures) console.error(`clean-results: ${failure}`);
  return failures.length === 0 ? 0 : 1;
}

process.exitCode = main(process.argv.slice(2));
