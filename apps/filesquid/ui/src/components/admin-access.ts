/**
 * The one role check of the admin views (UI-F2.21; UI-S-032, UI-S-021; PRD F-061): the Monitor
 * button and route, and the Logs row, are offered only to a caller whom the API grants the admin
 * routes, the desktop owner or a server administrator (owner decision 2026-10-31, verbatim:
 * "accept recommendations"). The UI learns it from the API's answer to the admin health route
 * (`GET /api/v1/admin/health`), with no contract field of its own: a `403` role refusal denies
 * the admin views, a successful answer grants them, and every other outcome (missing or invalid
 * credentials, an unavailable host, a network failure) leaves the role unknown, so the Monitor
 * keeps its sign-in form and its own error states. The desktop owner's credential is always
 * granted the admin routes, so the desktop owner always sees the Monitor.
 */
import { ApiError } from "../api/client";
import type { HostType } from "../host/descriptor";

/** What the UI knows of the caller's admin role. */
export type AdminAccess = "unknown" | "granted" | "denied";

/** Event an {@link AdminAccessStore} dispatches each time its state changes. */
export const ADMIN_ACCESS_EVENT = "change";

/** HTTP status of an authorisation refusal (API-S-004). */
const HTTP_FORBIDDEN = 403;

/** The part of the API client the probe needs (injectable in tests). */
export interface AdminProbeClient {
  /** Read the overall health from the admin health route. */
  adminHealth(): Promise<unknown>;
}

/**
 * The role an admin read shows. Pure.
 *
 * A `403` is a role refusal unless its reason is `invalid_credentials`, which API-S-004 also
 * answers with 403: an invalid token names no caller, so the sign-in form stays offered.
 *
 * @param error What the admin read rejected with, or undefined when it succeeded.
 * @returns `granted` after a success, `denied` after a role refusal, otherwise `unknown`.
 */
export function accessAfter(error: unknown): AdminAccess {
  if (error === undefined) return "granted";
  if (!(error instanceof ApiError) || error.status !== HTTP_FORBIDDEN) return "unknown";
  return error.reason === "invalid_credentials" ? "unknown" : "denied";
}

/**
 * Whether the shell offers the Monitor button and route. Pure.
 *
 * @param access What is known of the caller's admin role.
 * @returns False only for a caller the admin routes refused.
 */
export function offersMonitor(access: AdminAccess): boolean {
  return access !== "denied";
}

/**
 * Whether the shell offers the Logs row and the View in Logs links: the same role check as the
 * Monitor, on the desktop host only until the server host has its Logs view (UI-S-021). Pure.
 *
 * @param hostType The host the descriptor names, or undefined while it is unknown.
 * @param access What is known of the caller's admin role.
 * @returns True for the desktop owner.
 */
export function offersLogs(hostType: HostType | string | undefined, access: AdminAccess): boolean {
  return hostType === "desktop" && offersMonitor(access);
}

/**
 * Holds the caller's admin role for the shell, the Monitor and the file panel, and announces
 * every change with {@link ADMIN_ACCESS_EVENT}. One store per page.
 */
export class AdminAccessStore extends EventTarget {
  private current: AdminAccess = "unknown";
  private pending?: Promise<AdminAccess>;
  /** Bumped by every probe; an answer to an older probe is dropped. */
  private generation = 0;

  /**
   * @param client Reads the admin health route.
   * @param hasCredential Whether the page holds a credential to send. Without one the API can
   *   only answer `401`, which says nothing about the role, so no probe is sent.
   */
  constructor(
    private readonly client: AdminProbeClient,
    private readonly hasCredential: () => boolean,
  ) {
    super();
  }

  /** What is known now. */
  get state(): AdminAccess {
    return this.current;
  }

  /**
   * Ask the admin health route once whether the current credential is granted the admin
   * routes, for example at start and after a token is entered. Network I/O; without a
   * credential the role becomes unknown and nothing is sent.
   *
   * @returns The role the answer shows (the stored state, if a newer probe started meanwhile).
   */
  probe(): Promise<AdminAccess> {
    this.generation += 1;
    const generation = this.generation;
    if (!this.hasCredential()) {
      this.pending = undefined;
      this.set("unknown");
      return Promise.resolve(this.current);
    }
    const read = this.read(generation);
    this.pending = read;
    return read;
  }

  /**
   * The role once the probe in flight, if any, has answered; never sends a request itself.
   *
   * @returns The settled role.
   */
  settled(): Promise<AdminAccess> {
    return this.pending ?? Promise.resolve(this.current);
  }

  /** Record a role refusal another admin read met (the Monitor's own load or poll). */
  deny(): void {
    this.set("denied");
  }

  /** Read the admin health route and keep its answer unless a newer probe started. */
  private async read(generation: number): Promise<AdminAccess> {
    let error: unknown;
    try {
      await this.client.adminHealth();
    } catch (caught) {
      error = caught;
    }
    if (generation !== this.generation) return this.current;
    this.pending = undefined;
    this.set(accessAfter(error));
    return this.current;
  }

  /** Store a state and announce it when it changed. */
  private set(next: AdminAccess): void {
    if (next === this.current) return;
    this.current = next;
    this.dispatchEvent(new Event(ADMIN_ACCESS_EVENT));
  }
}
