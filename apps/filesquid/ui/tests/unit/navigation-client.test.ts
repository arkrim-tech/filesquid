/**
 * UI-F2.1b part 2: the live navigation source. `FileSquidClient.navigation()` reads
 * `GET /api/v1/navigation` with the node, cursor and limit as given (API-S-021), and
 * `ClientNavigationSource` answers the tree through it: every answer checked against the
 * contract, errors passed on unchanged (`not_found`, 503 `unavailable`), and one shared event
 * subscription turning `navigation.change`, `events.resync` and each channel open into
 * navigation changes.
 */
import { afterEach, describe, expect, it, vi } from "vitest";
import { ApiError, FileSquidClient, type ApiEvent } from "../../src/api/client";
import { ClientNavigationSource, type NavigationClient } from "../../src/navigation/client-source";
import type { NavigationChange } from "../../src/navigation/model";

afterEach(() => vi.restoreAllMocks());

const REPOSITORY = {
  nodes: [{ node: "r", kind: "repository", name: null, has_children: true }],
  ancestors: [],
  next_cursor: null,
};

/** A client whose fetch records each URL and answers `body` with `status`. */
function recordingClient(body: unknown = REPOSITORY, status = 200) {
  const urls: string[] = [];
  const client = new FileSquidClient({
    baseUrl: "",
    token: "abc",
    fetch: async (input) => {
      urls.push(String(input));
      return new Response(JSON.stringify(body), {
        status,
        headers: { "content-type": "application/json" },
      });
    },
  });
  return { client, urls };
}

describe("FileSquidClient.navigation", () => {
  it("reads the repository level with no query", async () => {
    const { client, urls } = recordingClient();
    expect(await client.navigation()).toEqual(REPOSITORY);
    expect(urls).toEqual(["/api/v1/navigation"]);
  });

  it("sends node, cursor and limit as given, encoded", async () => {
    const { client, urls } = recordingClient();
    await client.navigation({ node: "a b/c+é", cursor: "k=1&x", limit: 1 });
    expect(urls).toEqual(["/api/v1/navigation?node=a+b%2Fc%2B%C3%A9&cursor=k%3D1%26x&limit=1"]);
  });

  it("rejects with ApiError on a 503 while the host has no navigation service", async () => {
    const body = { code: "unavailable", message: "x", retryable: true };
    const { client } = recordingClient(body, 503);
    const error = await client.navigation().catch((e: unknown) => e);
    expect(error).toBeInstanceOf(ApiError);
    expect((error as ApiError).status).toBe(503);
    expect((error as ApiError).code).toBe("unavailable");
  });
});

/** A fake client: answers in order, and an event channel the test drives. */
function fakeClient(...answers: unknown[]) {
  const calls: unknown[] = [];
  const channel = { opened: 0, closed: 0 };
  let listener: ((event: ApiEvent) => void) | undefined;
  let onOpen: (() => void) | undefined;
  const client: NavigationClient = {
    navigation: async (query = {}) => {
      calls.push(query);
      const next = answers.length > 1 ? answers.shift() : answers[0];
      if (next instanceof Error) throw next;
      return next as never;
    },
    subscribe: (l, open) => {
      channel.opened += 1;
      listener = l;
      onOpen = open;
      return () => void (channel.closed += 1);
    },
  };
  return {
    client,
    calls,
    channel,
    emit: (event: ApiEvent) => listener?.(event),
    open: () => onOpen?.(),
  };
}

describe("ClientNavigationSource.list", () => {
  it("lists the repository level and passes parent, cursor and limit through", async () => {
    const fake = fakeClient(REPOSITORY);
    const source = new ClientNavigationSource(fake.client);
    expect(await source.list(null)).toEqual(REPOSITORY);
    await source.list("l1", "c2", 50);
    expect(fake.calls).toEqual([{}, { node: "l1", cursor: "c2", limit: 50 }]);
  });

  it("rejects an answer that breaks the contract with a TypeError, never showing it", async () => {
    const bad = { ...REPOSITORY, nodes: [{ node: "r", kind: "shelf", name: null }] };
    const source = new ClientNavigationSource(fakeClient(bad).client);
    await expect(source.list(null)).rejects.toBeInstanceOf(TypeError);
  });

  it("passes not_found and unavailable answers on unchanged", async () => {
    const gone = new ApiError(404, "/api/v1/navigation", { code: "not_found" });
    const off = new ApiError(503, "/api/v1/navigation", { code: "unavailable" });
    const source = new ClientNavigationSource(fakeClient(gone, off).client);
    await expect(source.list("x")).rejects.toBe(gone);
    await expect(source.list(null)).rejects.toBe(off);
  });
});

describe("ClientNavigationSource.subscribe", () => {
  /** Subscribe one recording listener. */
  function follow(source: ClientNavigationSource) {
    const changes: NavigationChange[] = [];
    const stop = source.subscribe((change) => changes.push(change));
    return { changes, stop };
  }

  it("turns navigation.change into the named parents, or all", () => {
    const fake = fakeClient(REPOSITORY);
    const { changes } = follow(new ClientNavigationSource(fake.client));
    fake.emit({ topic: "navigation.change", data: { parents: ["l1", "f1"] } });
    fake.emit({ topic: "navigation.change", data: { all: true } });
    expect(changes).toEqual([{ parents: ["l1", "f1"] }, { all: true }]);
  });

  it("reads the whole tree after events.resync and each time the channel opens", () => {
    const fake = fakeClient(REPOSITORY);
    const { changes } = follow(new ClientNavigationSource(fake.client));
    fake.emit({ topic: "events.resync", data: { missed: 2 } });
    fake.open();
    fake.open();
    expect(changes).toEqual([{ all: true }, { all: true }, { all: true }]);
  });

  it("reads the whole tree, with a warning, when a payload breaks the contract", () => {
    const warn = vi.spyOn(console, "warn").mockImplementation(() => undefined);
    const fake = fakeClient(REPOSITORY);
    const { changes } = follow(new ClientNavigationSource(fake.client));
    fake.emit({ topic: "navigation.change", data: { parents: [] } });
    expect(changes).toEqual([{ all: true }]);
    expect(warn).toHaveBeenCalled();
  });

  it("ignores other topics", () => {
    const fake = fakeClient(REPOSITORY);
    const { changes } = follow(new ClientNavigationSource(fake.client));
    fake.emit({ topic: "host.change", data: {} });
    fake.emit({ topic: "journal.entry", data: {} });
    expect(changes).toEqual([]);
  });

  it("shares one channel between listeners and closes it with the last one", () => {
    const fake = fakeClient(REPOSITORY);
    const source = new ClientNavigationSource(fake.client);
    const first = follow(source);
    const second = follow(source);
    expect(fake.channel.opened).toBe(1);
    fake.emit({ topic: "navigation.change", data: { parents: ["l1"] } });
    expect(first.changes).toEqual([{ parents: ["l1"] }]);
    expect(second.changes).toEqual([{ parents: ["l1"] }]);
    first.stop();
    expect(fake.channel.closed).toBe(0);
    second.stop();
    second.stop();
    expect(fake.channel.closed).toBe(1);
    follow(source);
    expect(fake.channel.opened).toBe(2);
  });

  it("reconnects with a new channel while followed (a token was entered), and not otherwise", () => {
    const fake = fakeClient(REPOSITORY);
    const source = new ClientNavigationSource(fake.client);
    source.reconnect();
    expect(fake.channel.opened).toBe(0);
    const { changes, stop } = follow(source);
    source.reconnect();
    expect(fake.channel).toEqual({ opened: 2, closed: 1 });
    fake.open();
    expect(changes).toEqual([{ all: true }]);
    stop();
    expect(fake.channel.closed).toBe(2);
  });

  it("logs and still answers listings when the channel cannot be opened", async () => {
    const error = vi.spyOn(console, "error").mockImplementation(() => undefined);
    const fake = fakeClient(REPOSITORY);
    fake.client.subscribe = () => {
      throw new ApiError(0, "/api/v1/events", { reason: "invalid_credentials" });
    };
    const source = new ClientNavigationSource(fake.client);
    const { stop } = follow(source);
    expect(error).toHaveBeenCalled();
    stop();
    expect(await source.list(null)).toEqual(REPOSITORY);
  });
});
