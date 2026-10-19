/**
 * Label tables: every visible noun comes from a table keyed by FileSquid's internal
 * names, so a distribution can supply its own vocabulary (ADR 031, FileSquid U-24).
 */

/** Keys every distribution's label table MUST define. */
export const REQUIRED_LABEL_KEYS = [
  "repository",
  "library",
  "libraries",
  "collection",
  "collections",
  "virtualCollection",
  "virtualCollections",
  "virtualExhibit",
  "virtualExhibits",
  "file",
  "files",
  "folder",
  "folders",
  "apiStatus",
  "apiUnavailable",
  "authMissingCredentials",
  "authInvalidCredentials",
  "authNotPermitted",
  "monitor",
  "monitorHealth",
  "monitorProcesses",
  "monitorNoProcesses",
  "monitorStorage",
  "monitorRates",
  "monitorJournal",
  "monitorFilter",
  "monitorSearch",
  "monitorExport",
  "monitorAdminToken",
  "monitorSignIn",
  "monitorUnavailable",
  "statusOk",
  "statusDegraded",
  "statusFailing",
  "processKind",
  "processState",
  "processProgress",
  "processRate",
  "processEta",
  "journalTime",
  "journalOperation",
  "journalOutcome",
  "journalActor",
  "storageFree",
  "storageWritable",
  "storageStore",
  "metricEntries",
  "metricWarnings",
  "metricErrors",
  "metricQueue",
  "theme",
  "themeSystem",
  "themeLight",
  "themeDark",
  "yes",
  "no",
  "skipToContent",
  "navigationMain",
  "navigationPinned",
  "navigationPin",
  "navigationPinItem",
  "navigationUnavailable",
  "navigationRemove",
  "navigationRemoveItem",
  "navigationMoveUp",
  "navigationMoveDown",
  "navigationBreadcrumb",
  "stateLoading",
  "stateEmpty",
  "stateError",
  "stateOffline",
  "stateDenied",
  "stateRetry",
  "hostIndicator",
  "hostDesktop",
  "hostDesktopBackup",
  "hostServer",
  "hostOperatorUnnamed",
  "hostServerAtRest",
  "hostBrowser",
  "hostServiceTitle",
  "hostNotEnrolled",
  "hostBackup",
  "hostSync",
  "hostOn",
  "hostOff",
  "hostServiceAt",
  "hostServiceCiphertext",
] as const;

/** One required label key. */
export type LabelKey = (typeof REQUIRED_LABEL_KEYS)[number];

/** A complete label table. */
export type LabelTable = Record<LabelKey, string>;

/**
 * Return the required keys a table is missing or leaves empty.
 *
 * @param table Candidate label table (for example parsed JSON).
 */
export function missingLabels(table: Record<string, unknown>): LabelKey[] {
  return REQUIRED_LABEL_KEYS.filter((key) => {
    const value = table[key];
    return typeof value !== "string" || value.trim() === "";
  });
}

/**
 * Fill the named placeholders of a label, for example `"Move {name} up"`, so a distribution
 * can order the words its language needs. Each value is inserted literally; a placeholder with
 * no value is left as written. Pure.
 *
 * @param template A label from the active table.
 * @param values Text for each placeholder, keyed by its name.
 * @returns The label with its placeholders filled.
 */
export function formatLabel(template: string, values: Readonly<Record<string, string>>): string {
  return template.replace(/\{([a-zA-Z]+)\}/g, (whole, name: string) =>
    Object.hasOwn(values, name) ? (values[name] ?? whole) : whole,
  );
}
