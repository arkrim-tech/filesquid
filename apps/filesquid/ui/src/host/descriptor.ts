/**
 * The host descriptor model (UI-S-004, UI-S-018): validation of what `GET /api/v1/host` and a
 * `host.change` event deliver, and the privacy statements worded from the label table
 * (ADR 032 Decision 6). Pure: no I/O, no DOM. The indicator element renders what
 * {@link describeHost} returns, so another view (a settings page, an about box) can word the
 * same facts without a second source of truth. The UI takes host type and operator name only
 * from the descriptor and never infers them (UI-S-004).
 */
import type { HostDescriptor } from "../api/client";
import { formatLabel, type LabelTable } from "../theme/labels";

export type { HostDescriptor } from "../api/client";

/** Host types the contract defines. */
export const HOST_TYPES = ["desktop", "server", "browser"] as const;

/** One host type. */
export type HostType = (typeof HOST_TYPES)[number];

/** Longest operator display name the contract allows (API-S-020). */
export const MAX_OPERATOR_NAME = 256;

/** The hosted backup and sync service as the indicator shows it. */
export type ServiceView =
  | { readonly enrolled: false; readonly text: string }
  | {
      readonly enrolled: true;
      readonly serviceHost: string;
      readonly backup: boolean;
      readonly sync: boolean;
      /** The ciphertext-only statement. */
      readonly text: string;
    };

/** Everything the indicator shows for one descriptor. */
export interface HostView {
  /** The host type the descriptor reported. */
  readonly hostType: HostType;
  /** The statements about the connected host, in reading order. */
  readonly statements: readonly string[];
  /** The hosted backup and sync service, shown apart from the host. */
  readonly service: ServiceView;
}

/** True for a plain object. */
function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

/** True when `text` contains a C0 or C1 control character or DEL. */
function hasControlCharacter(text: string): boolean {
  return [...text].some((char) => {
    const code = char.codePointAt(0) ?? 0;
    return code < 0x20 || (code >= 0x7f && code <= 0x9f);
  });
}

/** True for a valid operator name: 1 to 256 characters (code points) without controls. */
function validOperatorName(value: unknown): boolean {
  if (typeof value !== "string") return false;
  const length = [...value].length;
  if (length < 1 || length > MAX_OPERATOR_NAME) return false;
  return !hasControlCharacter(value);
}

/** A lowercase DNS host name: dot-separated labels of 1 to 63 letters, digits and hyphens. */
const DNS_HOST =
  /^(?=.{1,253}$)[a-z0-9]([a-z0-9-]{0,61}[a-z0-9])?(\.[a-z0-9]([a-z0-9-]{0,61}[a-z0-9])?)*$/;

/** True when `value` is an object with exactly the `allowed` keys (the contract is closed). */
function hasExactKeys(value: Record<string, unknown>, allowed: readonly string[]): boolean {
  const keys = Object.keys(value);
  return keys.length === allowed.length && allowed.every((key) => keys.includes(key));
}

const SERVICE_KEYS = ["enrolled", "backup_enabled", "sync_enabled", "service_host"] as const;
const DESCRIPTOR_KEYS = ["schema_version", "host_type", "operator_name", "hosted_service"] as const;

/** True for a valid hosted-service state: flags are booleans and agree with `enrolled`. */
function validService(value: unknown): boolean {
  if (!isRecord(value) || !hasExactKeys(value, SERVICE_KEYS)) return false;
  const { enrolled, backup_enabled: backup, sync_enabled: sync, service_host: host } = value;
  if (![enrolled, backup, sync].every((flag) => typeof flag === "boolean")) return false;
  if (enrolled === false) return !backup && !sync && host === null;
  return typeof host === "string" && DNS_HOST.test(host);
}

/** True when the operator name fits the host type: a name only on a server, else null. */
function validOperator(hostType: unknown, operator: unknown): boolean {
  return hostType === "server"
    ? operator === null || validOperatorName(operator)
    : operator === null;
}

/**
 * Check a value against the descriptor contract. A value that fails is never shown, so the
 * indicator cannot present a guessed host. Pure.
 *
 * @param value A parsed API answer or `host.change` payload.
 * @returns The descriptor, or `undefined` when the value does not satisfy the contract.
 */
export function parseHostDescriptor(value: unknown): HostDescriptor | undefined {
  if (!isRecord(value) || !hasExactKeys(value, DESCRIPTOR_KEYS)) return undefined;
  if (value.schema_version !== 1) return undefined;
  if (!HOST_TYPES.includes(value.host_type as HostType)) return undefined;
  if (!validOperator(value.host_type, value.operator_name)) return undefined;
  if (!validService(value.hosted_service)) return undefined;
  return value as unknown as HostDescriptor;
}

/** The statements about the connected host, worded from the label table. */
function hostStatements(descriptor: HostDescriptor, labels: LabelTable): string[] {
  const { backup_enabled: backup, sync_enabled: sync } = descriptor.hosted_service;
  switch (descriptor.host_type) {
    case "desktop":
      return backup || sync ? [labels.hostDesktop, labels.hostDesktopBackup] : [labels.hostDesktop];
    case "server": {
      const operator = descriptor.operator_name ?? labels.hostOperatorUnnamed;
      return [formatLabel(labels.hostServer, { operator }), labels.hostServerAtRest];
    }
    case "browser":
      return [labels.hostBrowser];
  }
}

/** The hosted service view, worded from the label table. */
function serviceView(descriptor: HostDescriptor, labels: LabelTable): ServiceView {
  const service = descriptor.hosted_service;
  if (!service.enrolled || service.service_host === null) {
    return { enrolled: false, text: labels.hostNotEnrolled };
  }
  return {
    enrolled: true,
    serviceHost: service.service_host,
    backup: service.backup_enabled,
    sync: service.sync_enabled,
    text: labels.hostServiceCiphertext,
  };
}

/**
 * Word a validated descriptor. Pure.
 *
 * @param descriptor A descriptor accepted by {@link parseHostDescriptor}.
 * @param labels The active label table.
 * @returns The host statements and the hosted service, kept apart.
 */
export function describeHost(descriptor: HostDescriptor, labels: LabelTable): HostView {
  return {
    hostType: descriptor.host_type,
    statements: hostStatements(descriptor, labels),
    service: serviceView(descriptor, labels),
  };
}
