/**
 * Reads and writes the caller's UI preferences through the API (UI-F2.8, API-S-011): the
 * {@link PreferencesSource} interface the {@link PreferencesService} uses, and
 * {@link ClientPreferencesSource}, which answers it with `GET` and `PUT /api/v1/preferences`
 * through `FileSquidClient`. A host that keeps preferences for no user answers 503, which the
 * service treats as "this host does not store preferences".
 */
import type { FileSquidClient } from "../api/client";
import { parsePreferences, type UserPreferences } from "./model";

/** Cancels a call that is no longer needed. */
export interface PreferencesCall {
  /** Cancels the call. */
  readonly signal?: AbortSignal;
}

/** What the preference service asks of the API. */
export interface PreferencesSource {
  /** Read the caller's preferences, or the defaults when none are saved. Reads. */
  read(call?: PreferencesCall): Promise<UserPreferences>;
  /** Replace the caller's preferences with a complete value; answers what was stored. Writes. */
  write(prefs: UserPreferences, call?: PreferencesCall): Promise<UserPreferences>;
}

/** The route (API-S-011). */
const PATH = "/api/v1/preferences";

/** The part of `FileSquidClient` the source needs (injectable in tests). */
export type PreferencesClient = Pick<FileSquidClient, "request">;

/** A {@link PreferencesSource} over `FileSquidClient`. */
export class ClientPreferencesSource implements PreferencesSource {
  /** @param client The API client (its transport adapter and token are already chosen). */
  constructor(private readonly client: PreferencesClient) {}

  /**
   * Read the preferences with `GET /api/v1/preferences`. Network I/O.
   *
   * @param call Abort signal.
   * @returns The checked preferences.
   * @throws ApiError when the API refuses (503 when the host stores none).
   * @throws TypeError when the answer breaks the contract.
   */
  async read(call: PreferencesCall = {}): Promise<UserPreferences> {
    return this.checked(await this.client.request<unknown>(PATH, { signal: call.signal }));
  }

  /**
   * Replace the preferences with `PUT /api/v1/preferences`. Network I/O.
   *
   * @param prefs The complete preferences.
   * @param call Abort signal.
   * @returns What the API stored.
   * @throws ApiError when the API refuses (400 for a bound, 409 when the host keeps no more).
   * @throws TypeError when the answer breaks the contract.
   */
  async write(prefs: UserPreferences, call: PreferencesCall = {}): Promise<UserPreferences> {
    const answer = await this.client.request<unknown>(PATH, {
      method: "PUT",
      body: prefs,
      signal: call.signal,
    });
    return this.checked(answer);
  }

  /** The answer as preferences, or a TypeError naming the route. */
  private checked(answer: unknown): UserPreferences {
    const prefs = parsePreferences(answer);
    if (!prefs) throw new TypeError(`${PATH} answered a value that breaks the contract`);
    return prefs;
  }
}

/**
 * A source that keeps the preferences in memory, as a host that stores them does. For tests and
 * fixtures; it answers what it was last given.
 */
export class MemoryPreferencesSource implements PreferencesSource {
  /** Every value written, oldest first. */
  readonly writes: UserPreferences[] = [];

  /** @param stored What the host holds now. */
  constructor(private stored: UserPreferences) {}

  /** @returns The stored preferences. */
  read(): Promise<UserPreferences> {
    return Promise.resolve(this.stored);
  }

  /**
   * @param prefs The value to keep.
   * @returns The kept value.
   */
  write(prefs: UserPreferences): Promise<UserPreferences> {
    this.stored = prefs;
    this.writes.push(prefs);
    return Promise.resolve(prefs);
  }
}
