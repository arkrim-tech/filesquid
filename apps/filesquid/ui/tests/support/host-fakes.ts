/**
 * Test double for the host half of the shell's client: answers `host()` with a descriptor and
 * keeps `subscribe()` quiet, so shell tests that are not about the host indicator can spread
 * it into their fake client.
 */
import type { HostClient, HostDescriptor } from "../../src/index";

/** A desktop descriptor with the hosted service not enrolled. */
export const DESKTOP_DESCRIPTOR: HostDescriptor = {
  schema_version: 1,
  host_type: "desktop",
  operator_name: null,
  hosted_service: {
    enrolled: false,
    backup_enabled: false,
    sync_enabled: false,
    service_host: null,
  },
};

/**
 * Build the host half of a fake client.
 *
 * @param descriptor What `host()` answers (default: desktop, not enrolled).
 * @returns `host()` and a `subscribe()` that opens nothing and closes nothing.
 */
export function hostFakes(descriptor: HostDescriptor = DESKTOP_DESCRIPTOR): HostClient {
  return {
    host: async () => descriptor,
    subscribe: () => () => undefined,
  };
}
