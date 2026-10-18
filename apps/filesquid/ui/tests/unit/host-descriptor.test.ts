/**
 * Unit tests for the host descriptor model (UI-S-004, UI-S-018): validation of what the API
 * and a `host.change` event deliver, and the privacy statements worded from the label table.
 */
import { describe, expect, it } from "vitest";
import { describeHost, parseHostDescriptor, type HostDescriptor } from "../../src/host/descriptor";
import labels from "../../src/theme/labels.en.json";

const NOT_ENROLLED = {
  enrolled: false,
  backup_enabled: false,
  sync_enabled: false,
  service_host: null,
};
const ENROLLED = {
  enrolled: true,
  backup_enabled: true,
  sync_enabled: false,
  service_host: "backup.filesquid.tech",
};

/** A descriptor with overrides. */
function descriptor(over: Record<string, unknown> = {}): HostDescriptor {
  return {
    schema_version: 1,
    host_type: "desktop",
    operator_name: null,
    hosted_service: NOT_ENROLLED,
    ...over,
  } as HostDescriptor;
}

describe("parseHostDescriptor", () => {
  it("accepts each host type", () => {
    for (const host_type of ["desktop", "server", "browser"]) {
      expect(parseHostDescriptor(descriptor({ host_type }))?.host_type).toBe(host_type);
    }
  });

  it("accepts an enrolled service with its host name", () => {
    expect(parseHostDescriptor(descriptor({ hosted_service: ENROLLED }))?.hosted_service).toEqual(
      ENROLLED,
    );
  });

  it.each([
    ["null", null],
    ["a string", "desktop"],
    ["a wrong schema version", descriptor({ schema_version: 2 })],
    ["an unknown host type", descriptor({ host_type: "cloud" })],
    ["a missing host type", descriptor({ host_type: undefined })],
    ["a numeric operator", descriptor({ operator_name: 7 })],
    ["an empty operator name", descriptor({ operator_name: "" })],
    ["an operator name with a control character", descriptor({ operator_name: "a\u0007b" })],
    ["an overlong operator name", descriptor({ operator_name: "x".repeat(257) })],
    ["no hosted service", descriptor({ hosted_service: undefined })],
    ["a non-boolean flag", descriptor({ hosted_service: { ...NOT_ENROLLED, sync_enabled: "no" } })],
    [
      "backup on while not enrolled",
      descriptor({ hosted_service: { ...NOT_ENROLLED, backup_enabled: true } }),
    ],
    [
      "a service host while not enrolled",
      descriptor({ hosted_service: { ...NOT_ENROLLED, service_host: "a.b" } }),
    ],
    [
      "enrolled without a service host",
      descriptor({ hosted_service: { ...ENROLLED, service_host: null } }),
    ],
  ])("rejects %s", (_name, value) => {
    expect(parseHostDescriptor(value)).toBeUndefined();
  });
});

describe("describeHost", () => {
  it("words the desktop statement and not-enrolled state from the label table", () => {
    const view = describeHost(descriptor(), labels);
    expect(view.hostType).toBe("desktop");
    expect(view.statements).toEqual([labels.hostDesktop]);
    expect(view.service).toEqual({ enrolled: false, text: labels.hostNotEnrolled });
  });

  it("adds the on-device encryption sentence on a desktop with backup or sync on", () => {
    for (const hosted_service of [
      ENROLLED,
      { ...ENROLLED, backup_enabled: false, sync_enabled: true },
    ]) {
      const view = describeHost(descriptor({ hosted_service }), labels);
      expect(view.statements).toEqual([labels.hostDesktop, labels.hostDesktopBackup]);
    }
  });

  it("does not add that sentence when enrolled with backup and sync both off", () => {
    const off = { ...ENROLLED, backup_enabled: false };
    expect(describeHost(descriptor({ hosted_service: off }), labels).statements).toEqual([
      labels.hostDesktop,
    ]);
  });

  it("inserts the operator name into the server statement, then states the at-rest duty", () => {
    const view = describeHost(
      descriptor({ host_type: "server", operator_name: "Garderen Family" }),
      labels,
    );
    expect(view.statements).toEqual([
      "Your files are stored on a server run by Garderen Family. That operator can read them. Other users see only what they are permitted to.",
      labels.hostServerAtRest,
    ]);
  });

  it("uses the unnamed-operator label when the operator has no name", () => {
    const view = describeHost(descriptor({ host_type: "server" }), labels);
    expect(view.statements[0]).toContain(labels.hostOperatorUnnamed);
    expect(view.statements[0]).toContain("can read them");
    expect(view.statements[0]).not.toContain("{");
  });

  it("inserts an operator name literally, including braces and markup", () => {
    const name = "<b>{operator}</b>";
    const view = describeHost(descriptor({ host_type: "server", operator_name: name }), labels);
    expect(view.statements[0]).toContain(`run by ${name}.`);
  });

  it("words the browser host statement", () => {
    expect(describeHost(descriptor({ host_type: "browser" }), labels).statements).toEqual([
      labels.hostBrowser,
    ]);
  });

  it("shows backup and sync separately with the service host and the ciphertext statement", () => {
    const view = describeHost(descriptor({ hosted_service: ENROLLED }), labels);
    expect(view.service).toEqual({
      enrolled: true,
      serviceHost: "backup.filesquid.tech",
      backup: true,
      sync: false,
      text: labels.hostServiceCiphertext,
    });
  });

  it("never copies operator text onto a host that has no operator", () => {
    const view = describeHost(
      descriptor({ host_type: "desktop", operator_name: "Someone" }),
      labels,
    );
    expect(view.statements.join(" ")).not.toContain("Someone");
  });
});
