/**
 * The files the preview measurements open (Task UI-F3.1, NFR-002): 60 display-size images
 * (1280 x 720 PNG, each different, so no cache can answer for another), 6 originals of 12
 * megapixels (4000 x 3000 JPEG, the "high decoded-pixel count" case) and 4 text files of 150
 * KiB (the ranged read of the first 64 KiB). Everything is generated from a seed: PNGs by a
 * small encoder in Node, JPEGs by the browser's own encoder, so there is no image dependency.
 */
import { crc32, deflateSync } from "node:zlib";
import type { Browser } from "@playwright/test";
import type { MemoryFile } from "../../src/files/memory-source";
import { seeded } from "./fixtures";

/** Display-size images. */
export const SMALL_COUNT = 60;
/** Originals of 12 megapixels. */
export const LARGE_COUNT = 6;
/** Text files. */
export const TEXT_COUNT = 4;

/** Width and height of a display-size image. */
const SMALL = { width: 1280, height: 720 } as const;
/** Width and height of a 12-megapixel original. */
const LARGE = { width: 4000, height: 3000 } as const;

/** A PNG chunk: length, type, data and the CRC of type and data. */
function chunk(type: string, data: Buffer): Buffer {
  const body = Buffer.concat([Buffer.from(type, "latin1"), data]);
  const head = Buffer.alloc(4);
  head.writeUInt32BE(data.length);
  const tail = Buffer.alloc(4);
  tail.writeUInt32BE(crc32(body));
  return Buffer.concat([head, body, tail]);
}

/**
 * A PNG of a colour gradient that differs with the seed, with sparse noise so it is not a
 * flat picture.
 *
 * @param seed - Which image.
 * @returns The PNG file's bytes.
 */
export function png(seed: number): Buffer {
  const { width, height } = SMALL;
  const random = seeded(seed + 1);
  const row = width * 3 + 1;
  const raw = Buffer.alloc(row * height);
  for (let y = 0; y < height; y += 1) {
    const at = y * row;
    for (let x = 0; x < width; x += 1) {
      const noise = random() < 0.02 ? Math.floor(random() * 255) : 0;
      raw[at + 1 + x * 3] = ((x * 255) / width + seed * 7 + noise) & 255;
      raw[at + 2 + x * 3] = ((y * 255) / height + seed * 13) & 255;
      raw[at + 3 + x * 3] = (((x + y) * 255) / (width + height) + seed * 29) & 255;
    }
  }
  const header = Buffer.alloc(13);
  header.writeUInt32BE(width, 0);
  header.writeUInt32BE(height, 4);
  header.set([8, 2, 0, 0, 0], 8);
  return Buffer.concat([
    Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]),
    chunk("IHDR", header),
    chunk("IDAT", deflateSync(raw, { level: 1 })),
    chunk("IEND", Buffer.alloc(0)),
  ]);
}

/**
 * A 12-megapixel JPEG made by the browser's encoder: a gradient with seeded rectangles.
 *
 * @param browser - A browser to draw in.
 * @param seed - Which image.
 * @returns The JPEG file's bytes.
 */
async function jpeg(browser: Browser, seed: number): Promise<Buffer> {
  const page = await browser.newPage();
  try {
    const base64 = await page.evaluate(
      async ({ size, seed: s }) => {
        const canvas = new OffscreenCanvas(size.width, size.height);
        const draw = canvas.getContext("2d") as OffscreenCanvasRenderingContext2D;
        const gradient = draw.createLinearGradient(0, 0, size.width, size.height);
        gradient.addColorStop(0, `hsl(${(s * 47) % 360} 70% 45%)`);
        gradient.addColorStop(1, `hsl(${(s * 47 + 140) % 360} 70% 60%)`);
        draw.fillStyle = gradient;
        draw.fillRect(0, 0, size.width, size.height);
        let state = s * 2654435761;
        for (let i = 0; i < 1500; i += 1) {
          state = (state * 1664525 + 1013904223) >>> 0;
          draw.fillStyle = `hsl(${state % 360} 60% ${30 + (state % 40)}% / 0.6)`;
          draw.fillRect(
            state % size.width,
            (state >>> 8) % size.height,
            60 + (state % 200),
            40 + (state % 160),
          );
        }
        const blob = await canvas.convertToBlob({ type: "image/jpeg", quality: 0.85 });
        const bytes = new Uint8Array(await blob.arrayBuffer());
        let text = "";
        for (let i = 0; i < bytes.length; i += 0x8000)
          text += String.fromCharCode(...bytes.subarray(i, i + 0x8000));
        return btoa(text);
      },
      { size: LARGE, seed },
    );
    return Buffer.from(base64, "base64");
  } finally {
    await page.close();
  }
}

/** The folder of display-size images and the name of image `n` (1-based). */
export const smallName = (n: number): string => `img-${String(n).padStart(3, "0")}.png`;
/** The name of original `n` (1-based). */
export const largeName = (n: number): string => `photo-${n}.jpg`;
/** The name of text file `n` (1-based). */
export const textName = (n: number): string => `note-${n}.txt`;

/** A text file of about 150 KiB whose first line names it. */
function text(n: number): string {
  const lines = Array.from(
    { length: 2400 },
    (_, i) => `line ${i} of note ${n}: harbour masterplan survey`,
  );
  return `NOTE ${n}\n${lines.join("\n")}\nTAIL-MARKER`;
}

/** What the preview specs open. */
export interface PreviewFiles {
  /** Every file, with the folder in its path. */
  readonly files: MemoryFile[];
  /** Total bytes of the display-size images. */
  readonly smallBytes: number;
  /** Total bytes of the originals. */
  readonly largeBytes: number;
}

let cached: Promise<PreviewFiles> | undefined;

/**
 * Generate the preview files once per run.
 *
 * @param browser - A browser, for the JPEG encoder.
 * @returns The files.
 */
export function previewFiles(browser: Browser): Promise<PreviewFiles> {
  cached ??= (async () => {
    const small = Array.from({ length: SMALL_COUNT }, (_, i) => ({
      path: `small/${smallName(i + 1)}`,
      content: png(i + 1),
    }));
    const large = [] as { path: string; content: Buffer }[];
    for (let i = 0; i < LARGE_COUNT; i += 1) {
      large.push({ path: `large/${largeName(i + 1)}`, content: await jpeg(browser, i + 1) });
    }
    const texts = Array.from({ length: TEXT_COUNT }, (_, i) => ({
      path: `text/${textName(i + 1)}`,
      content: text(i + 1),
    }));
    const bytes = (list: { content: Buffer | string }[]) =>
      list.reduce((sum, file) => sum + Buffer.byteLength(file.content), 0);
    return {
      files: [...small, ...large, ...texts],
      smallBytes: bytes(small),
      largeBytes: bytes(large),
    };
  })();
  return cached;
}
