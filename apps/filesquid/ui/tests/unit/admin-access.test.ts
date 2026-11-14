/**
 * Unit tests for the one role check of the admin views (UI-F2.21; UI-S-032, UI-S-021): the
 * Monitor button and route, and the Logs row, are offered only to a caller the API grants the
 * admin routes, learned from the answer to `GET /api/v1/admin/health` with no contract field.
 */
import { describe, expect, it } from "vitest";
import {
  ADMIN_ACCESS_EVENT,
  AdminAccessStore,
  accessAfter,
  offersLogs,
  offersMonitor,
  type AdminAccess,
} from "../../src/index";
import { ApiError } from "../../src/api/client";

/** A 403 role refusal of the admin routes (API-S-004). */
const refused = () => new ApiError(403, "/api/v1/admin/health", { reason: "not_permitted" });

/** A probe client whose admin health read answers with `answer` and counts the calls. */
function probeClient(answer: () => Promise<unknown>) {
  let calls = 0;
  return {
    client: {
      adminHealth: () => {
        calls += 1;
        return answer();
      },
    },
    calls: () => calls,
  };
}

describe("accessAfter", () => {
  it("grants the admin views after a successful admin read", () => {
    expect(accessAfter(undefined)).toBe("granted");
  });

  it("denies them after a 403 role refusal, with or without a reason", () => {
    expect(accessAfter(refused())).toBe("denied");
    expect(accessAfter(new ApiError(403, "/api/v1/admin/health"))).toBe("denied");
  });

  it("keeps the role unknown after invalid or missing credentials, a 503 or a network failure", () => {
    const invalid = new ApiError(403, "/x", { reason: "invalid_credentials" });
    const missing = new ApiError(401, "/x", { reason: "missing_credentials" });
    expect(accessAfter(invalid)).toBe("unknown");
    expect(accessAfter(missing)).toBe("unknown");
    expect(accessAfter(new ApiError(503, "/x", { code: "unavailable" }))).toBe("unknown");
    expect(accessAfter(new TypeError("Failed to fetch"))).toBe("unknown");
  });
});

describe("offersMonitor and offersLogs", () => {
  it.each([
    ["granted", true],
    ["unknown", true],
    ["denied", false],
  ] as const)("offers the Monitor for %s: %s", (access: AdminAccess, offered) => {
    expect(offersMonitor(access)).toBe(offered);
  });

  it("offers the Logs row on the desktop host only, and never to a refused caller", () => {
    expect(offersLogs("desktop", "granted")).toBe(true);
    expect(offersLogs("desktop", "unknown")).toBe(true);
    expect(offersLogs("desktop", "denied")).toBe(false);
    expect(offersLogs("server", "granted")).toBe(false);
    expect(offersLogs(undefined, "granted")).toBe(false);
  });
});

describe("AdminAccessStore", () => {
  it("starts unknown and sends no probe without a credential", async () => {
    const fake = probeClient(() => Promise.resolve({}));
    const store = new AdminAccessStore(fake.client, () => false);
    expect(store.state).toBe("unknown");
    expect(await store.probe()).toBe("unknown");
    expect(fake.calls()).toBe(0);
  });

  it("grants the desktop owner's credential and announces the change", async () => {
    const fake = probeClient(() => Promise.resolve({ status: "ok" }));
    const store = new AdminAccessStore(fake.client, () => true);
    const seen: AdminAccess[] = [];
    store.addEventListener(ADMIN_ACCESS_EVENT, () => seen.push(store.state));
    expect(await store.probe()).toBe("granted");
    expect(seen).toEqual(["granted"]);
    expect(fake.calls()).toBe(1);
  });

  it("denies after a 403 and settles to the probe in flight without a second call", async () => {
    const fake = probeClient(() => Promise.reject(refused()));
    const store = new AdminAccessStore(fake.client, () => true);
    const pending = store.probe();
    expect(await store.settled()).toBe("denied");
    expect(await pending).toBe("denied");
    expect(await store.settled()).toBe("denied");
    expect(fake.calls()).toBe(1);
  });

  it("drops a stale answer when a newer probe was started", async () => {
    const answers: (() => void)[] = [];
    let first = true;
    const store = new AdminAccessStore(
      {
        adminHealth: () =>
          new Promise((resolve, reject) => {
            const deny = first;
            first = false;
            answers.push(() => (deny ? reject(refused()) : resolve({})));
          }),
      },
      () => true,
    );
    const older = store.probe();
    const newer = store.probe();
    answers[1]?.();
    expect(await newer).toBe("granted");
    answers[0]?.();
    await older;
    expect(store.state).toBe("granted");
  });

  it("records a role refusal the Monitor met on its own read", () => {
    const store = new AdminAccessStore(probeClient(() => Promise.resolve({})).client, () => true);
    let changes = 0;
    store.addEventListener(ADMIN_ACCESS_EVENT, () => (changes += 1));
    store.deny();
    store.deny();
    expect(store.state).toBe("denied");
    expect(changes).toBe(1);
  });
});
