/**
 * The media library of the packaged-window content-handle tests (Task API-F1.3; ADR 030 S1
 * item (a), API-S-018): an OCFL 1.1 storage root with one object holding a VP8 WebM video of
 * at least 100 MB that plays for longer than the 300 s idle window of a content handle, and
 * a PNG image. The desktop's test-instrumented build opens it as library
 * `E2E_LIBRARY_ID` when `FILESQUID_E2E_LIBRARY` names it (`src/test_fixture.rs`).
 *
 * The media are generated with `ffmpeg` (libvpx and png encoders), so no binary fixture is
 * checked in. The OCFL files are written directly, following the OCFL 1.1 layout the engine
 * reads: the root declaration, the object declaration, `inventory.json` with a sha256
 * sidecar in the object root and in `v1/`, and the content under `v1/content/`.
 */
import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import {
  closeSync,
  mkdirSync,
  openSync,
  readSync,
  renameSync,
  statSync,
  writeFileSync,
} from "node:fs";
import path from "node:path";

/** The library ID the desktop opens the media library as (`src/test_fixture.rs`). */
export const E2E_LIBRARY_ID = "5e2e0000-0000-4000-8000-00000000e2e0";

/** The OCFL object that holds the media. */
export const E2E_OBJECT_ID = "urn:uuid:5e2e0000-0000-4000-8000-0000000000aa";

/** The video's logical path. */
export const VIDEO_PATH = "holiday.webm";

/** The image's logical path. */
export const IMAGE_PATH = "photo.png";

/** The image's size in pixels. */
export const IMAGE_SIZE = { width: 320, height: 240 };

/** Seconds the video plays: longer than the 300 s idle window plus a margin. */
export const VIDEO_SECONDS = 400;

/** The smallest acceptable video: 100 MB. */
export const MIN_VIDEO_BYTES = 100 * 1000 * 1000;

/** Bytes read per step while hashing a file. */
const HASH_CHUNK = 8 * 1024 * 1024;

/**
 * Run `ffmpeg` with `args`, failing with its message when it is missing or fails.
 *
 * @param args The ffmpeg arguments.
 */
function ffmpeg(args) {
  const result = spawnSync("ffmpeg", ["-hide_banner", "-loglevel", "error", "-y", ...args], {
    encoding: "utf8",
  });
  if (result.error || result.status !== 0) {
    const reason = result.error ? result.error.message : result.stderr;
    throw new Error(`ffmpeg (with libvpx) is required for the content-handle tests: ${reason}`);
  }
}

/**
 * The lowercase hex sha256 digest of the file at `file`, read in bounded steps.
 *
 * @param file A file path.
 * @returns The digest.
 */
function sha256File(file) {
  const hash = createHash("sha256");
  const buffer = Buffer.alloc(HASH_CHUNK);
  const fd = openSync(file, "r");
  try {
    const steps = Math.ceil(statSync(file).size / HASH_CHUNK) + 1;
    for (let step = 0; step < steps; step += 1) {
      const read = readSync(fd, buffer, 0, HASH_CHUNK, null);
      if (read === 0) break;
      hash.update(buffer.subarray(0, read));
    }
  } finally {
    closeSync(fd);
  }
  return hash.digest("hex");
}

/**
 * Encode the video and the image into `content`.
 *
 * @param content The object's `v1/content` directory.
 */
function encodeMedia(content) {
  const video = path.join(content, VIDEO_PATH);
  const source = `testsrc2=size=640x360:rate=25,noise=alls=60:allf=t`;
  ffmpeg([
    "-f",
    "lavfi",
    "-i",
    source,
    "-t",
    String(VIDEO_SECONDS),
    "-c:v",
    "libvpx",
    "-b:v",
    "2100k",
    "-minrate",
    "2100k",
    "-maxrate",
    "2100k",
    "-bufsize",
    "4200k",
    "-deadline",
    "realtime",
    "-cpu-used",
    "8",
    "-an",
    video,
  ]);
  const size = statSync(video).size;
  if (size < MIN_VIDEO_BYTES) {
    throw new Error(`the encoded video has ${size} bytes, fewer than ${MIN_VIDEO_BYTES}`);
  }
  const { width, height } = IMAGE_SIZE;
  const image = path.join(content, IMAGE_PATH);
  ffmpeg(["-f", "lavfi", "-i", `testsrc2=size=${width}x${height}`, "-frames:v", "1", image]);
}

/**
 * Write `inventory` and its sha256 sidecar into `dir`.
 *
 * @param dir A directory.
 * @param inventory The inventory text.
 */
function writeInventory(dir, inventory) {
  writeFileSync(path.join(dir, "inventory.json"), inventory);
  const digest = createHash("sha256").update(inventory).digest("hex");
  writeFileSync(path.join(dir, "inventory.json.sha256"), `${digest} inventory.json\n`);
}

/**
 * Build the media library under `dir` and answer its storage root. The root is written
 * under a temporary name and renamed into place last, so a half-built root is never used.
 *
 * @param dir An empty directory of the test run.
 * @returns The storage root's path.
 */
export function buildMediaLibrary(dir) {
  const building = path.join(dir, "media-root.building");
  const object = path.join(building, "objects", "media");
  const content = path.join(object, "v1", "content");
  mkdirSync(content, { recursive: true });
  encodeMedia(content);
  const files = [VIDEO_PATH, IMAGE_PATH].map((name) => ({
    name,
    digest: sha256File(path.join(content, name)),
  }));
  const manifest = Object.fromEntries(files.map((f) => [f.digest, [`v1/content/${f.name}`]]));
  const state = Object.fromEntries(files.map((f) => [f.digest, [f.name]]));
  const inventory = JSON.stringify(
    {
      id: E2E_OBJECT_ID,
      type: "https://ocfl.io/1.1/spec/#inventory",
      digestAlgorithm: "sha256",
      head: "v1",
      manifest,
      versions: {
        v1: { created: "2026-10-18T00:00:00Z", message: "Packaged-window media", state },
      },
    },
    null,
    2,
  );
  writeFileSync(path.join(building, "0=ocfl_1.1"), "ocfl_1.1\n");
  writeFileSync(path.join(object, "0=ocfl_object_1.1"), "ocfl_object_1.1\n");
  writeInventory(object, inventory);
  writeInventory(path.join(object, "v1"), inventory);
  const root = path.join(dir, "media-root");
  renameSync(building, root);
  return root;
}
