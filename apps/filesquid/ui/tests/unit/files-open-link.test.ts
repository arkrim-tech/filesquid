/**
 * Unit tests for the deep link of the files route (UI-F2.16; UI-S-021, UI-S-020): the Logs view's
 * View provenance and Show file open one file's panel on a tab through
 * `#/files/<library>/urn:uuid:<file>/original?open=<file>&tab=<tab>`. The query is not part of
 * the location, and a link that names no known tab or a file that is not a UUID opens nothing.
 */
import { describe, expect, it } from "vitest";
import { openFileHash, parseLocationHash, parseOpenRequest } from "../../src/files/model";

const FILE = "1d0e88b4-0000-4000-8000-00000000a14c";

describe("the open-file link", () => {
  it("is the file's own original in its library, with the file and the tab in the query", () => {
    expect(openFileHash("lib-1", FILE, "provenance")).toBe(
      `#/files/lib-1/urn%3Auuid%3A${FILE}/original?open=${FILE}&tab=provenance`,
    );
  });

  it("opens the file's panel on its first tab when no tab is named", () => {
    expect(openFileHash("lib-1", FILE)).toBe(
      `#/files/lib-1/urn%3Auuid%3A${FILE}/original?open=${FILE}`,
    );
  });

  it("is still the folder of the file's original for the location", () => {
    const hash = openFileHash("lib 1", FILE, "details");
    expect(parseLocationHash(hash)).toEqual({
      library_id: "lib 1",
      object_id: `urn:uuid:${FILE}`,
      prefix: "original",
    });
  });

  it("reads the file and the tab back", () => {
    expect(parseOpenRequest(openFileHash("lib-1", FILE, "provenance"))).toEqual({
      file: FILE,
      tab: "provenance",
    });
    expect(parseOpenRequest(openFileHash("lib-1", FILE))).toEqual({ file: FILE });
  });

  it("reads nothing from an address with no query, an unknown tab or a file that is no UUID", () => {
    expect(parseOpenRequest(`#/files/lib-1/urn%3Auuid%3A${FILE}/original`)).toBeUndefined();
    expect(parseOpenRequest("#/files/lib-1/o/original?open=not-a-uuid")).toBeUndefined();
    expect(parseOpenRequest(`#/files/lib-1/o/original?open=${FILE}&tab=secrets`)).toEqual({
      file: FILE,
    });
    expect(parseOpenRequest(`#/monitor?open=${FILE}`)).toBeUndefined();
  });
});
