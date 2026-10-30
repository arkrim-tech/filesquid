/**
 * Unit tests for editing a version's label (UI-F2.11, UI-S-009, PRD F-065): `ClientFileSource`
 * sends `PUT /api/v1/files/labels` as a command (API-F2.5, API-F2.13) with the object's head
 * as the expected revision and `null` to clear a label, and `MemoryFileSource` answers like
 * the route: one new version, an unchanged label refused 409, a stale head refused 409, a bad
 * label refused 400.
 */
import { describe, expect, it, vi } from "vitest";
import { ApiError } from "../../src/api/client";
import { MemoryFileSource } from "../../src/files/memory-source";
import { ClientFileSource, type FileClient } from "../../src/files/source";

const OBJECT = { library_id: "lib", object_id: "obj" };

/** A client whose `command` records its calls and answers a committed label. */
function commandClient() {
  const result = { library_id: "lib", object_id: "obj", version: "v8", created: "t" };
  const command = vi.fn(async () => ({
    operation_id: "op",
    error: null,
    kind: "committed",
    result,
  }));
  return { command } as unknown as FileClient & { command: typeof command };
}

describe("ClientFileSource.setVersionLabel", () => {
  it("PUTs the command with the label, the version and the head as expected revision", async () => {
    const c = commandClient();
    const commit = await new ClientFileSource(c).setVersionLabel(OBJECT, "v5", "Client review", {
      revision: "v7",
    });
    expect(commit.version).toBe("v8");
    expect(c.command).toHaveBeenCalledWith(
      "/api/v1/files/labels",
      expect.objectContaining({ expected_revision: "v7", scope: "library:lib" }),
      { library_id: "lib", object_id: "obj", version: "v5", label: "Client review" },
      { signal: undefined, method: "PUT" },
    );
  });

  it("sends null to clear a label", async () => {
    const c = commandClient();
    await new ClientFileSource(c).setVersionLabel(OBJECT, "v5", null, { revision: "v7" });
    const [, , input] = c.command.mock.calls[0] as unknown as [string, object, { label: unknown }];
    expect(input.label).toBeNull();
  });

  it("passes the abort signal on", async () => {
    const c = commandClient();
    const abort = new AbortController();
    await new ClientFileSource(c).setVersionLabel(OBJECT, "v5", "x", {
      revision: "v7",
      signal: abort.signal,
    });
    const [, , , options] = c.command.mock.calls[0] as unknown as [
      string,
      object,
      object,
      { signal: AbortSignal; method: string },
    ];
    expect(options.signal).toBe(abort.signal);
    expect(options.method).toBe("PUT");
  });

  it("rejects a conflict outcome as ApiError 409", async () => {
    const command = vi.fn(async () => ({
      operation_id: "op",
      error: null,
      kind: "conflict",
      result: null,
    }));
    const c = { command } as unknown as FileClient;
    await expect(
      new ClientFileSource(c).setVersionLabel(OBJECT, "v5", "x", { revision: "v1" }),
    ).rejects.toMatchObject({ status: 409, code: "conflict" });
  });
});

/** The contract's cap of labelled versions per object (API-F2.5), written here on its own. */
const LABEL_CAP = 1000;

/** Label versions v1 to v1000 one after another, so 1,000 versions carry a label. */
async function fillToCap(s: MemoryFileSource): Promise<void> {
  for (let n = 1; n <= LABEL_CAP; n += 1) {
    await s.setVersionLabel(OBJECT, `v${n}`, `Label ${n}`, { revision: s.head });
  }
}

/** A source holding one file with two versions. */
function memory(): MemoryFileSource {
  const s = new MemoryFileSource({
    library_id: "lib",
    object_id: "obj",
    files: [{ path: "a.txt", content: "one" }],
  });
  return s;
}

describe("MemoryFileSource.setVersionLabel", () => {
  it("labels an earlier version and commits one new version", async () => {
    const s = memory();
    const commit = await s.setVersionLabel(OBJECT, "v1", "First draft", { revision: s.head });
    expect(commit.version).toBe("v2");
    expect(s.head).toBe("v2");
    const versions = await s.versions();
    expect(versions.versions[0]?.label).toBe("First draft");
    expect(versions.versions[1]?.label).toBeNull();
  });

  it("clears a label with null", async () => {
    const s = memory();
    await s.setVersionLabel(OBJECT, "v1", "First draft", { revision: "v1" });
    await s.setVersionLabel(OBJECT, "v1", null, { revision: "v2" });
    expect((await s.versions()).versions[0]?.label).toBeNull();
  });

  it("refuses a label that would not change anything, without a version", async () => {
    const s = memory();
    await s.setVersionLabel(OBJECT, "v1", "Same", { revision: "v1" });
    await expect(
      s.setVersionLabel(OBJECT, "v1", "Same", { revision: s.head }),
    ).rejects.toMatchObject({ status: 409, code: "conflict" });
    await expect(s.setVersionLabel(OBJECT, "v2", null, { revision: s.head })).rejects.toMatchObject(
      { status: 409 },
    );
    expect(s.head).toBe("v2");
  });

  it("refuses a stale head as conflict and an invalid label as invalid_request", async () => {
    const s = memory();
    await expect(s.setVersionLabel(OBJECT, "v1", "x", { revision: "v0" })).rejects.toMatchObject({
      status: 409,
      code: "conflict",
    });
    for (const bad of ["", "   ", "a\u0007b", "x".repeat(513)]) {
      await expect(s.setVersionLabel(OBJECT, "v1", bad, { revision: "v1" })).rejects.toMatchObject({
        status: 400,
        code: "invalid_request",
      });
    }
  });

  it("answers not_found for a version that does not exist", async () => {
    const s = memory();
    const refusal = s.setVersionLabel(OBJECT, "v9", "x", { revision: "v1" });
    await expect(refusal).rejects.toBeInstanceOf(ApiError);
    await expect(refusal).rejects.toMatchObject({ status: 404, code: "not_found" });
  });

  it("refuses a label past the 1,000 labelled versions an object may have, as conflict, publishing nothing", async () => {
    const s = memory();
    await fillToCap(s);
    const next = `v${LABEL_CAP + 1}`;
    const head = s.head;
    const count = (await s.versions()).versions.length;
    await expect(
      s.setVersionLabel(OBJECT, next, "One too many", { revision: head }),
    ).rejects.toMatchObject({ status: 409, code: "conflict" });
    expect(s.head).toBe(head);
    expect((await s.versions()).versions).toHaveLength(count);
    // Changing or clearing a label that exists is still allowed at the cap.
    await s.setVersionLabel(OBJECT, "v1", "Renamed", { revision: s.head });
    await s.setVersionLabel(OBJECT, "v2", null, { revision: s.head });
    await s.setVersionLabel(OBJECT, next, "Fits again", { revision: s.head });
  });

  it("refuses a labelled restore past the cap, publishing nothing, and still restores without a label", async () => {
    const s = memory();
    await s.remove(OBJECT, "a.txt", { revision: "v1" });
    // v2 removed the file; label v2 and v3.. so that 1,000 versions carry a label.
    for (let n = 1; n <= LABEL_CAP; n += 1) {
      await s.setVersionLabel(OBJECT, `v${n}`, `Label ${n}`, { revision: s.head });
    }
    const head = s.head;
    const count = (await s.versions()).versions.length;
    await expect(
      s.restore(OBJECT, "v1", ["a.txt"], { revision: head, label: "One too many" }),
    ).rejects.toMatchObject({ status: 409, code: "conflict" });
    expect(s.head).toBe(head);
    expect((await s.versions()).versions).toHaveLength(count);
    const restored = await s.restore(OBJECT, "v1", ["a.txt"], { revision: head });
    expect(restored.version).toBe(`v${count + 1}`);
  });
});
