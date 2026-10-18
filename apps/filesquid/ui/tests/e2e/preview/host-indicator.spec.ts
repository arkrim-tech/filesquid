/**
 * Production-artifact proof of the host privacy indicator (UI-S-004, UI-S-018): the FileSquid
 * UI built by `vite build` and served by `vite preview`, with the descriptor route and the
 * event channel answered by the test. Strict DOM assertions: the custom element is defined,
 * every word comes from the active label table, computed token values are exact in light and
 * dark, and there are no console errors.
 */
import { expect, test, type Page, type WebSocketRoute } from "@playwright/test";
import {
  activeLabels,
  expectElementsDefined,
  label,
  rgbOf,
  setScheme,
  trackErrors,
} from "../support/strict-dom";

const labels = activeLabels();
const SERVICE = "backup.filesquid.tech";
const NONE = { enrolled: false, backup_enabled: false, sync_enabled: false, service_host: null };

/** A hosted-service state. */
const enrolled = (backup: boolean, sync: boolean) => ({
  enrolled: true,
  backup_enabled: backup,
  sync_enabled: sync,
  service_host: SERVICE,
});

/** A descriptor with overrides. */
const descriptor = (over: Record<string, unknown> = {}) => ({
  schema_version: 1,
  host_type: "desktop",
  operator_name: null,
  hosted_service: NONE,
  ...over,
});

/** What the test controls about the API: the descriptor answers and the open event channel. */
interface Api {
  /** Number of `GET /api/v1/host` calls so far. */
  reads: () => number;
  /** Push one event down the open event channel. */
  send: (event: object) => void;
}

/**
 * Answer health, the descriptor route and the event channel.
 *
 * @param page - The page.
 * @param answers - Descriptor answers in call order; the last repeats. A number is an HTTP
 *   status to answer instead.
 * @param delayMs - Delay before each descriptor answer.
 */
async function serve(page: Page, answers: (object | number)[], delayMs = 0): Promise<Api> {
  let reads = 0;
  let socket: WebSocketRoute | undefined;
  await page.route("**/api/v1/health", (route) =>
    route.fulfill({ json: { schema_version: 1, status: "ok", product: "filesquid" } }),
  );
  await page.route("**/api/v1/host", async (route) => {
    const answer = answers[Math.min(reads, answers.length - 1)];
    reads += 1;
    if (delayMs) await new Promise((resolve) => setTimeout(resolve, delayMs));
    if (typeof answer === "number") {
      await route.fulfill({
        status: answer,
        json: { code: "internal", message: "x", retryable: false },
      });
      return;
    }
    await route.fulfill({ json: answer });
  });
  await page.routeWebSocket(/\/api\/v1\/events$/, (ws) => {
    socket = ws;
  });
  return {
    reads: () => reads,
    send: (event) => socket?.send(JSON.stringify(event)),
  };
}

/** Texts of the elements matching `selector` inside the indicator. */
const inIndicator = (page: Page, selector: string) => page.locator(`fs-host-indicator ${selector}`);

test("the indicator is a defined custom element in the shell header, named from the label table", async ({
  page,
}) => {
  const errors = trackErrors(page);
  await serve(page, [descriptor()]);
  await page.goto("/");
  await expectElementsDefined(page, ["fs-host-indicator", "fs-app-shell", "fs-states"]);
  const group = page.getByRole("group", { name: label(labels, "hostIndicator") });
  await expect(group).toBeVisible();
  await expect(page.locator("fs-app-shell header fs-host-indicator")).toHaveCount(1);
  await expect(page.locator("fs-host-indicator")).toHaveAttribute("data-state", "ready");
  expect(errors).toEqual([]);
});

test("the desktop host says only you can read your files, and shows the not-enrolled state", async ({
  page,
}) => {
  const errors = trackErrors(page);
  await serve(page, [descriptor()]);
  await page.goto("/");
  await expect(inIndicator(page, ".statement")).toHaveText([label(labels, "hostDesktop")]);
  await expect(inIndicator(page, ".service h3")).toHaveText(label(labels, "hostServiceTitle"));
  await expect(inIndicator(page, ".not-enrolled")).toHaveText(label(labels, "hostNotEnrolled"));
  await expect(inIndicator(page, '[data-on="true"]')).toHaveCount(0);
  await expect(inIndicator(page, "dl")).toHaveCount(0);
  expect(errors).toEqual([]);
});

test("a server names its operator, who can read the files, and states the at-rest duty", async ({
  page,
}) => {
  const errors = trackErrors(page);
  await serve(page, [descriptor({ host_type: "server", operator_name: "Garderen Family" })]);
  await page.goto("/");
  await expect(inIndicator(page, ".statement")).toHaveText([
    "Your files are stored on a server run by Garderen Family. That operator can read them. Other users see only what they are permitted to.",
    label(labels, "hostServerAtRest"),
  ]);
  expect(errors).toEqual([]);
});

test("a server without an operator name uses the unnamed-operator label", async ({ page }) => {
  const errors = trackErrors(page);
  await serve(page, [descriptor({ host_type: "server" })]);
  await page.goto("/");
  const first = inIndicator(page, ".statement").first();
  await expect(first).toContainText(label(labels, "hostOperatorUnnamed"));
  await expect(first).toContainText("That operator can read them.");
  expect(errors).toEqual([]);
});

test("the browser host says files are decrypted only in this browser", async ({ page }) => {
  const errors = trackErrors(page);
  await serve(page, [descriptor({ host_type: "browser", hosted_service: enrolled(true, true) })]);
  await page.goto("/");
  await expect(inIndicator(page, ".statement")).toHaveText([label(labels, "hostBrowser")]);
  await expect(inIndicator(page, ".service dd")).toHaveText([
    label(labels, "hostOn"),
    label(labels, "hostOn"),
  ]);
  await expect(inIndicator(page, ".ciphertext")).toHaveText(label(labels, "hostServiceCiphertext"));
  expect(errors).toEqual([]);
});

test("enrolled backup and sync are shown apart from the host, as ciphertext only", async ({
  page,
}) => {
  const errors = trackErrors(page);
  await serve(page, [descriptor({ hosted_service: enrolled(true, false) })]);
  await page.goto("/");
  await expect(inIndicator(page, ".statement")).toHaveText([
    label(labels, "hostDesktop"),
    label(labels, "hostDesktopBackup"),
  ]);
  await expect(inIndicator(page, ".service dt")).toHaveText([
    label(labels, "hostBackup"),
    label(labels, "hostSync"),
  ]);
  await expect(inIndicator(page, ".service dd")).toHaveText([
    label(labels, "hostOn"),
    label(labels, "hostOff"),
  ]);
  await expect(inIndicator(page, ".service-host")).toHaveText(
    `${label(labels, "hostServiceAt")} ${SERVICE}`,
  );
  await expect(inIndicator(page, ".service .statement")).toHaveCount(0);
  expect(errors).toEqual([]);
});

test("a host.change event updates the indicator without a reload or another read", async ({
  page,
}) => {
  const errors = trackErrors(page);
  const api = await serve(page, [descriptor()]);
  await page.goto("/");
  await expect(inIndicator(page, ".not-enrolled")).toBeVisible();
  // The read at start, then the read when the event channel opens (events before it are lost).
  await expect.poll(api.reads).toBe(2);
  await page.evaluate(() => ((window as unknown as { marker: string }).marker = "same page"));
  api.send({ topic: "host.change", data: descriptor({ hosted_service: enrolled(false, true) }) });
  await expect(inIndicator(page, ".service dd")).toHaveText([
    label(labels, "hostOff"),
    label(labels, "hostOn"),
  ]);
  expect(await page.evaluate(() => (window as unknown as { marker: string }).marker)).toBe(
    "same page",
  );
  expect(api.reads()).toBe(2);
  expect(errors).toEqual([]);
});

test("events.resync makes the indicator read the descriptor again", async ({ page }) => {
  const errors = trackErrors(page);
  const answers = [descriptor(), descriptor(), descriptor({ host_type: "browser" })];
  const api = await serve(page, answers);
  await page.goto("/");
  await expect(inIndicator(page, ".statement")).toHaveText([label(labels, "hostDesktop")]);
  await expect.poll(api.reads).toBe(2);
  api.send({ topic: "events.resync", data: { missed: 2 } });
  await expect(inIndicator(page, ".statement")).toHaveText([label(labels, "hostBrowser")]);
  expect(api.reads()).toBe(3);
  expect(errors).toEqual([]);
});

test("loading shows no host until the descriptor arrives", async ({ page }) => {
  const errors = trackErrors(page);
  await serve(page, [descriptor()], 800);
  await page.goto("/");
  const indicator = page.locator("fs-host-indicator");
  await expect(indicator).toHaveAttribute("data-state", "loading");
  await expect(inIndicator(page, "fs-states")).toContainText(label(labels, "stateLoading"));
  await expect(inIndicator(page, ".statement")).toHaveCount(0);
  await expect(indicator).toHaveAttribute("data-state", "ready");
  expect(errors).toEqual([]);
});

test("a failed read shows the error state and never a guessed host, then Retry recovers", async ({
  page,
}) => {
  const errors = trackErrors(page, /Failed to load resource/);
  // Two failing reads: the one at start and the one when the event channel opens.
  await serve(page, [500, 500, descriptor()]);
  await page.goto("/");
  const indicator = page.locator("fs-host-indicator");
  await expect(indicator).toHaveAttribute("data-state", "error");
  await expect(inIndicator(page, "fs-states")).toContainText(label(labels, "stateError"));
  await expect(inIndicator(page, ".statement")).toHaveCount(0);
  await expect(indicator).not.toHaveAttribute("data-host-type", /.+/);
  await inIndicator(page, "fs-states button").click();
  await expect(indicator).toHaveAttribute("data-state", "ready");
  await expect(inIndicator(page, ".statement")).toHaveText([label(labels, "hostDesktop")]);
  expect(errors).toEqual([]);
});

test("computed token values are exact in light and dark", async ({ page }) => {
  const errors = trackErrors(page);
  await serve(page, [descriptor()]);
  const style = () =>
    page.locator("fs-host-indicator section").evaluate((el) => {
      const css = getComputedStyle(el);
      return { background: css.backgroundColor, color: css.color, border: css.borderTopColor };
    });
  await setScheme(page, "light");
  await page.goto("/");
  await expect(page.locator("fs-host-indicator")).toHaveAttribute("data-state", "ready");
  expect(await style()).toEqual({
    background: rgbOf("#f5f5f5"),
    color: rgbOf("#111111"),
    border: rgbOf("#e0e0e0"),
  });
  await setScheme(page, "dark");
  expect(await style()).toEqual({
    background: rgbOf("#1c1c1c"),
    color: rgbOf("#e0e0e0"),
    border: rgbOf("#2a2a2a"),
  });
  expect(errors).toEqual([]);
});

test("the indicator is reachable by keyboard and has a visible focus outline", async ({ page }) => {
  const errors = trackErrors(page);
  await serve(page, [descriptor()]);
  await page.goto("/");
  await expect(page.locator("fs-host-indicator")).toHaveAttribute("data-state", "ready");
  const focused = () =>
    page.evaluate(() => {
      let active: Element | null = document.activeElement;
      while (active?.shadowRoot?.activeElement) active = active.shadowRoot.activeElement;
      return active?.tagName === "SECTION" && active.getAttribute("role") === "group";
    });
  let reached = false;
  for (let presses = 0; presses < 12 && !reached; presses += 1) {
    await page.keyboard.press("Tab");
    reached = await focused();
  }
  expect(reached).toBe(true);
  const outline = await page.locator("fs-host-indicator section").evaluate((el) => {
    const css = getComputedStyle(el);
    return { style: css.outlineStyle, width: css.outlineWidth, color: css.outlineColor };
  });
  expect(outline).toEqual({ style: "solid", width: "2px", color: rgbOf("#0b7a32") });
  expect(errors).toEqual([]);
});
