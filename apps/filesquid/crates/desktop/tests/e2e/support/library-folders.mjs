/**
 * The owner-chosen library folders of the packaged-window library-route tests (Task DE-1.1).
 * A native folder dialog cannot be driven by WebDriver, so the test-instrumented app registers
 * these folders at launch under fixed location IDs (`src/test_fixture.rs`).
 */
import { existsSync, mkdirSync, readdirSync, rmSync } from "node:fs";
import path from "node:path";

/** The location ID of the empty folder a new library is created in. */
export const CREATE_LOCATION = "loc-e2e-create";

/** The location ID of the empty folder a snapshot is copied into. */
export const COPY_LOCATION = "loc-e2e-copy";

/** The location ID of the media library's storage root. */
export const MEDIA_LOCATION = "loc-e2e-media";

/** The library ID the snapshot copy is opened as. */
export const COPY_LIBRARY_ID = "5e2e0000-0000-4000-8000-00000000c0b1";

/**
 * Make the folders `create` and `copy` under `<dir>/locations`, emptied first, so a reused
 * run directory starts from empty folders too. Only these two generated folders are cleared.
 *
 * @param dir The run directory.
 * @returns The folder that holds them, for `FILESQUID_E2E_LOCATIONS`.
 */
export function buildLocationFolders(dir) {
  const root = path.join(dir, "locations");
  for (const name of ["create", "copy"]) {
    const folder = path.join(root, name);
    rmSync(folder, { recursive: true, force: true });
    mkdirSync(folder, { recursive: true });
  }
  return root;
}

/**
 * Remove the version inventory and its sidecar from the head version of every object in the
 * OCFL storage root at `root` (a W010 head, which only the consented repair may fix).
 *
 * @param root The storage root's folder.
 * @returns The head version directories changed.
 */
export function removeHeadInventories(root) {
  const changed = [];
  const objects = path.join(root, "objects");
  for (const name of existsSync(objects) ? readdirSync(objects) : []) {
    const head = path.join(objects, name, "v1");
    if (existsSync(path.join(head, "inventory.json"))) {
      rmSync(path.join(head, "inventory.json"));
      rmSync(path.join(head, "inventory.json.sha256"), { force: true });
      changed.push(head);
    }
  }
  return changed;
}
