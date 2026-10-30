/**
 * The File info rows of the Details panel (UI-F2.3, UI-S-003, UI-S-031 mockup 35): the storage
 * facts of `GET /api/v1/files/details` with the version total of `GET /api/v1/files/versions`,
 * worded from the label table as property rows of a label over a value. Storage facts only,
 * each owned by the OCFL module (UI-S-003); the stored name and the logical path are never
 * shown (UI-S-019), and the API never answers a content location (API-S-005). Pure.
 */
import { formatLabel, type LabelTable } from "../theme/labels";
import type { FileHistorySize, ObjectVersions } from "./history-source";
import { formatSize, type FileDetails } from "./model";
import { shortDigest, shortIdentifier, versionCountText } from "./versions";

/** One property row of a file panel table. */
export interface PropertyRow {
  /** Stable key of the row, for tests and `data-key`. */
  readonly key: string;
  /** The property, as the user sees it. */
  readonly label: string;
  /** The value shown. */
  readonly value: string;
  /** The text a copy puts on the clipboard; the value when absent. */
  readonly copy?: string;
  /** True for identifiers, codes, digests and extensions, set in the mono face. */
  readonly mono?: boolean;
  /**
   * A label that is a property key (a CURIE), set in the mono face: `owner` in `--fs-primary`
   * for an owner-asserted value, `extracted` in the secondary colour for a value read from
   * the file (UI-S-031 mockup 28). A plain label when absent.
   */
  readonly labelStyle?: "owner" | "extracted";
}

/** Most fixity algorithms shown besides the content digest. */
export const MAX_FIXITY_ROWS = 8;

/** The algorithm the mockup shows first. */
const PREFERRED = "sha256";

/** An algorithm name as people write it: `sha256` is `SHA-256`, `md5` is `MD5`. Pure. */
export function algorithmName(algorithm: string): string {
  return algorithm.toUpperCase().replace(/^SHA(\d)/, "SHA-$1");
}

/**
 * The digest rows of one file: SHA-256 first when known, then the content digest, then the
 * other fixity values the inventory records, each algorithm once. Pure.
 *
 * @param details The storage facts.
 * @param labels The active label table.
 * @returns One row per algorithm, at most {@link MAX_FIXITY_ROWS} plus the content digest.
 */
export function digestRows(details: FileDetails, labels: LabelTable): PropertyRow[] {
  const fixity = new Map<string, string>();
  const preferred = details.fixity[PREFERRED]?.[0];
  if (preferred && details.digest_algorithm !== PREFERRED) fixity.set(PREFERRED, preferred);
  for (const [algorithm, digests] of Object.entries(details.fixity)) {
    const first = digests[0];
    const known = algorithm === details.digest_algorithm || fixity.has(algorithm);
    if (first && !known && fixity.size < MAX_FIXITY_ROWS) fixity.set(algorithm, first);
  }
  const values = new Map<string, string>();
  if (fixity.has(PREFERRED)) values.set(PREFERRED, fixity.get(PREFERRED) as string);
  values.set(details.digest_algorithm, details.digest);
  for (const [algorithm, digest] of fixity) values.set(algorithm, digest);
  return [...values].map(([algorithm, digest]) => ({
    key: `digest-${algorithm}`,
    label: formatLabel(labels.filePanelDigest, { algorithm: algorithmName(algorithm) }),
    value: shortDigest(digest),
    copy: digest,
    mono: true,
  }));
}

/** A time in UTC as a short date and a 24-hour clock, or the text unchanged. */
function utcTime(iso: string, locale?: string): string {
  const time = new Date(iso);
  if (Number.isNaN(time.getTime())) return iso;
  return new Intl.DateTimeFormat(locale, {
    year: "numeric",
    month: "short",
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit",
    hourCycle: "h23",
    timeZone: "UTC",
  }).format(time);
}

/** The OCFL version row: "v4 of 4 · 16 Oct 2026, 11:05 UTC". */
function versionRow(
  details: FileDetails,
  versions: ObjectVersions,
  labels: LabelTable,
  locale?: string,
): PropertyRow {
  const record = versions.versions.find((v) => v.version === details.version);
  const created = record?.created ?? details.last_change.created;
  const value = formatLabel(labels.filePanelVersionValue, {
    version: details.version,
    total: new Intl.NumberFormat(locale).format(versions.versions.length),
    time: utcTime(created, locale),
  });
  return { key: "version", label: labels.filePanelVersion, value };
}

/** The size row: "6.2 MB · 6,481,920 bytes". */
function sizeRow(details: FileDetails, labels: LabelTable, locale?: string): PropertyRow {
  const value = formatLabel(labels.filePanelSizeValue, {
    size: formatSize(details.size, labels, locale),
    bytes: new Intl.NumberFormat(locale).format(details.size),
  });
  return { key: "size", label: labels.filePanelSize, value, copy: String(details.size) };
}

/**
 * The history size row (UI-F2.11, PRD F-065: storage used by history is reported per file):
 * "{size} · {count} versions", the bytes the file's history keeps, each content once. Pure.
 *
 * @param history The history size of the file.
 * @param labels The active label table.
 * @param locale BCP 47 tag for numbers.
 * @returns The row.
 */
function historySizeRow(
  history: Pick<FileHistorySize, "versions" | "bytes">,
  labels: LabelTable,
  locale?: string,
): PropertyRow {
  const value = formatLabel(labels.filePanelHistorySizeValue, {
    size: formatSize(history.bytes, labels, locale),
    versions: versionCountText(history.versions, labels, locale),
  });
  return { key: "history-size", label: labels.filePanelHistorySize, value };
}

/**
 * The File info rows of one file version, in the order of mockup 35, with the history size
 * after the size. Pure.
 *
 * @param details The storage facts of the version.
 * @param versions Every version of its object (for the total and the version's time).
 * @param labels The active label table.
 * @param locale BCP 47 tag for numbers and dates.
 * @param history The file's history size; the row is left out when it could not be read.
 * @returns The rows; the extension row only when the file has an extension hint.
 */
export function detailRows(
  details: FileDetails,
  versions: ObjectVersions,
  labels: LabelTable,
  locale?: string,
  history?: Pick<FileHistorySize, "versions" | "bytes">,
): PropertyRow[] {
  const extension: PropertyRow[] = details.type_hint
    ? [
        {
          key: "extension",
          label: labels.filePanelExtension,
          value: `.${details.type_hint}`,
          mono: true,
        },
      ]
    : [];
  return [
    { key: "name", label: labels.filePanelOriginalName, value: details.display_name },
    ...extension,
    { key: "format", label: labels.filePanelFormat, value: details.media_type },
    sizeRow(details, labels, locale),
    ...(history ? [historySizeRow(history, labels, locale)] : []),
    ...digestRows(details, labels),
    {
      key: "object",
      label: labels.filePanelObject,
      value: shortIdentifier(details.object_id),
      copy: details.object_id,
      mono: true,
    },
    versionRow(details, versions, labels, locale),
  ];
}
