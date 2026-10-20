/**
 * The owner-chosen library folders of the packaged-window library-route tests (Task DE-1.1).
 * A native folder dialog cannot be driven by WebDriver, so the test-instrumented app registers
 * these folders at launch under fixed location IDs (`src/test_fixture.rs`).
 */
import { mkdirSync } from "node:fs";
import path from "node:path";

/** The location ID of the empty folder a new library is created in. */
export const CREATE_LOCATION = "loc-e2e-create";

/** The location ID of the empty folder a snapshot is copied into. */
export const COPY_LOCATION = "loc-e2e-copy";

/** The location ID of the media library's storage root. */
export const MEDIA_LOCATION = "loc-e2e-media";

/**
 * Make the empty folders `create` and `copy` under `<dir>/locations`.
 *
 * @param dir The run directory.
 * @returns The folder that holds them, for `FILESQUID_E2E_LOCATIONS`.
 */
export function buildLocationFolders(dir) {
  const root = path.join(dir, "locations");
  for (const name of ["create", "copy"]) {
    mkdirSync(path.join(root, name), { recursive: true });
  }
  return root;
}
