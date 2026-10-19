/**
 * Packaged-window proof of content handles for media elements (Task API-F1.3; API-S-018,
 * DE-S-005; ADR 030 Decision 5 and S1 item (a)): in a real Tauri webview, the app page mints
 * a content handle with its per-launch token, and `<img>` and `<video>` load file versions of
 * the media library through `filesquid://` content URLs that carry no token. The video is
 * larger than 100 MB, so it can only play through capped `Range` reads (each answer carries at
 * most 16 MiB); the test seeks, and keeps it playing for longer than the handle's 300 s idle
 * window.
 *
 * Each `browser.execute` body runs in the app page, so it cannot use this module's scope; the
 * values it needs are passed in as arguments.
 */
import { browser, expect } from "@wdio/globals";
import {
  E2E_LIBRARY_ID,
  E2E_OBJECT_ID,
  IMAGE_PATH,
  IMAGE_SIZE,
  MIN_VIDEO_BYTES,
  VIDEO_PATH,
  VIDEO_SECONDS,
} from "../support/media-library.mjs";

/**
 * The privacy headers of a content-route answer (API-S-018) that page script can read. The
 * app page's origin differs from the scheme's, so CORS shows it only safelisted response
 * headers; `Referrer-Policy`, `X-Content-Type-Options` and `Content-Security-Policy` are
 * asserted on the scheme answer itself in `tests/integration/api_protocol.rs`.
 */
const VISIBLE_PRIVACY_HEADERS = {
  "cache-control": "private, no-store",
  "content-type": "application/json",
};

/** Wall-clock seconds the video keeps playing: past the 300 s idle window. */
const PLAY_SECONDS = 320;

/**
 * The scheme base URL the app page uses on this platform (ADR 030 Decision 3).
 *
 * @returns `http://filesquid.localhost` on Windows, `filesquid://localhost` elsewhere.
 */
function apiBase() {
  return process.platform === "win32" ? "http://filesquid.localhost" : "filesquid://localhost";
}

/**
 * Mint a content handle for `path` at v1 from the app page, with its per-launch token.
 *
 * @param path The file's logical path.
 * @returns The status and the answer body.
 */
async function mint(path) {
  return browser.execute(
    async (base, library, object, file) => {
      const token = window.__FILESQUID_LAUNCH_TOKEN__;
      const response = await fetch(`${base}/api/v1/content-handles`, {
        method: "POST",
        headers: { Authorization: `Bearer ${token}`, "Content-Type": "application/json" },
        body: JSON.stringify({ library_id: library, object_id: object, version: "v1", path: file }),
      });
      return { status: response.status, body: await response.json() };
    },
    apiBase(),
    E2E_LIBRARY_ID,
    E2E_OBJECT_ID,
    path,
  );
}

describe("content handles for media elements in a packaged window", () => {
  before(async () => {
    await browser.waitUntil(
      async () => (await browser.execute(() => document.readyState)) === "complete",
      { timeout: 60000, timeoutMsg: "the app page did not load" },
    );
  });

  it("mints a handle whose URL carries no token", async () => {
    const minted = await mint(VIDEO_PATH);
    expect(minted.status).toBe(200);
    expect(minted.body.handle).toMatch(/^[A-Za-z0-9_-]{43}$/);
    expect(minted.body.url).toBe(`/api/v1/content/${minted.body.handle}`);
    const token = await browser.execute(() => window.__FILESQUID_LAUNCH_TOKEN__);
    expect(minted.body.url.includes(token)).toBe(false);
  });

  it("reads a range of the 100 MB video through the handle without a token", async () => {
    const minted = await mint(VIDEO_PATH);
    const result = await browser.execute(
      async (base, url) => {
        const head = await fetch(`${base}${url}`, { method: "HEAD" });
        const length = Number(head.headers.get("Content-Length"));
        const tail = await fetch(`${base}${url}`, { headers: { Range: "bytes=-100" } });
        const past = await fetch(`${base}${url}`, { headers: { Range: `bytes=${length}-` } });
        return {
          length,
          type: head.headers.get("Content-Type"),
          headRanges: head.headers.get("Accept-Ranges"),
          headBytes: (await head.arrayBuffer()).byteLength,
          tailRanges: tail.headers.get("Accept-Ranges"),
          tail: tail.status,
          range: tail.headers.get("Content-Range"),
          bytes: (await tail.arrayBuffer()).byteLength,
          past: past.status,
          pastRange: past.headers.get("Content-Range"),
        };
      },
      apiBase(),
      minted.body.url,
    );
    expect(result.length).toBeGreaterThanOrEqual(MIN_VIDEO_BYTES);
    expect(result.type).toBe("video/webm");
    expect(result.headRanges).toBe("bytes");
    expect(result.headBytes).toBe(0);
    expect(result.tailRanges).toBe("bytes");
    expect(result.tail).toBe(206);
    expect(result.range).toBe(`bytes ${result.length - 100}-${result.length - 1}/${result.length}`);
    expect(result.bytes).toBe(100);
    expect(result.past).toBe(416);
    expect(result.pastRange).toBe(`bytes */${result.length}`);
  });

  it("refuses an unknown handle with the one identical denial", async () => {
    const result = await browser.execute(
      async (base, names) => {
        const url = `${base}/api/v1/content/${"A".repeat(43)}`;
        const headers = (response) =>
          Object.fromEntries(names.map((name) => [name, response.headers.get(name)]));
        const get = await fetch(url);
        const head = await fetch(url, { method: "HEAD" });
        const other = await fetch(`${base}/api/v1/content/not+a+handle`);
        return {
          status: get.status,
          body: await get.text(),
          headers: headers(get),
          headStatus: head.status,
          headHeaders: headers(head),
          headBytes: (await head.arrayBuffer()).byteLength,
          otherStatus: other.status,
          otherBody: await other.text(),
        };
      },
      apiBase(),
      Object.keys(VISIBLE_PRIVACY_HEADERS),
    );
    expect(result.status).toBe(403);
    const error = JSON.parse(result.body);
    expect(error.code).toBe("forbidden");
    expect(error.reason).toBe("invalid_credentials");
    expect(error.retryable).toBe(false);
    expect(result.headers).toEqual(VISIBLE_PRIVACY_HEADERS);
    expect(result.headStatus).toBe(403);
    expect(result.headHeaders).toEqual(VISIBLE_PRIVACY_HEADERS);
    expect(result.headBytes).toBe(0);
    expect(result.otherStatus).toBe(403);
    expect(result.otherBody).toBe(result.body);
  });

  it("loads an <img> through a content handle", async () => {
    const minted = await mint(IMAGE_PATH);
    const size = await browser.execute(
      async (base, url) => {
        const image = document.createElement("img");
        const loaded = new Promise((resolve, reject) => {
          image.onload = resolve;
          image.onerror = () => reject(new Error("the image did not load"));
        });
        image.src = `${base}${url}`;
        document.body.appendChild(image);
        await loaded;
        return { width: image.naturalWidth, height: image.naturalHeight };
      },
      apiBase(),
      minted.body.url,
    );
    expect(size).toEqual(IMAGE_SIZE);
  });

  // WebKitGTK (Linux) cannot play a <video> from the custom scheme: its GStreamer source takes
  // only http(s) and blob URIs, so after one probe range it reports MEDIA_ERR_SRC_NOT_SUPPORTED
  // (measured 2026-10-19, Task API-F1.3). ADR 030 S1 (a) records it as a known limitation for
  // release 1; the MediaSource player of UI-F3.3 is the planned fix (owner decision). This test
  // pins that measurement, so it fails, and must be revisited, once WebKitGTK plays.
  const webkitgtk = process.platform === "linux";

  (webkitgtk ? it : it.skip)(
    "records that WebKitGTK cannot play a <video> from the custom scheme",
    async () => {
      const minted = await mint(VIDEO_PATH);
      const outcome = await browser.execute(
        async (base, url) => {
          const video = document.createElement("video");
          video.muted = true;
          const settled = new Promise((resolve) => {
            const timer = setTimeout(
              () => resolve({ code: null, network: video.networkState }),
              20000,
            );
            video.onloadedmetadata = () => {
              clearTimeout(timer);
              resolve({ code: null, loaded: true });
            };
            video.onerror = () => {
              clearTimeout(timer);
              resolve({ code: video.error?.code ?? null, network: video.networkState });
            };
          });
          video.src = `${base}${url}`;
          document.body.appendChild(video);
          return settled;
        },
        apiBase(),
        minted.body.url,
      );
      // MEDIA_ERR_SRC_NOT_SUPPORTED with NETWORK_NO_SOURCE. The key is not `error`: the
      // WebDriver protocol would read a returned `error` member as a failed command.
      expect(outcome).toEqual({ code: 4, network: 3 });
    },
  );

  (webkitgtk ? it.skip : it)(
    "plays the <video>, seeks, and keeps playing past the 300 s idle window",
    async function () {
      this.timeout((PLAY_SECONDS + 180) * 1000);
      const minted = await mint(VIDEO_PATH);
      const ready = await browser.execute(
        async (base, url) => {
          const video = document.createElement("video");
          video.id = "e2e-video";
          video.muted = true;
          video.preload = "auto";
          const loaded = new Promise((resolve, reject) => {
            video.onloadedmetadata = resolve;
            video.onerror = () => reject(new Error(`video error ${video.error?.code}`));
          });
          video.src = `${base}${url}`;
          document.body.appendChild(video);
          await loaded;
          const seeked = new Promise((resolve) => (video.onseeked = resolve));
          video.currentTime = 200;
          await seeked;
          const sought = video.currentTime;
          const rewound = new Promise((resolve) => (video.onseeked = resolve));
          video.currentTime = 0;
          await rewound;
          await video.play();
          return { duration: video.duration, sought };
        },
        apiBase(),
        minted.body.url,
      );
      expect(ready.duration).toBeGreaterThan(VIDEO_SECONDS - 2);
      expect(ready.sought).toBeGreaterThanOrEqual(199);
      await browser.pause(PLAY_SECONDS * 1000);
      const played = await browser.execute(() => {
        const video = document.getElementById("e2e-video");
        return { time: video.currentTime, code: video.error?.code ?? null, paused: video.paused };
      });
      expect(played.code).toBe(null);
      expect(played.paused).toBe(false);
      expect(played.time).toBeGreaterThan(300);
    },
  );
});
