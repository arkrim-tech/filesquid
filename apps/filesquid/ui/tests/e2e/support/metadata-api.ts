/**
 * Fakes of the metadata property routes the Description tab reads and writes (UI-F2.4,
 * API-S-035) for the production-build Playwright tests: `GET files/metadata`,
 * `PUT files/metadata/statements`, `POST files/metadata/removals`, `GET files/metadata/terms`
 * and `GET files/metadata/namespaces`, answering mockup 28's eight statements of
 * `facade-study.jpg` (one of them extracted), one `exif:` statement the tab does not list, and
 * a registry with the RiC-O extension namespace and one custom namespace. The set and remove
 * routes take the contract's plain bodies (no envelope) and publish the next version; a value
 * the record already holds is a 409 `conflict`; an unknown `dcterms:` name is a 400. These
 * routes are registered after `servePanel`, so they take precedence.
 */
import type { Page, Route } from "@playwright/test";
import { PANEL_LIBRARY, PANEL_OBJECT } from "./file-panel-api";

/** The fixture file's id, from its object `urn:uuid:` (API-S-026). */
export const METADATA_FILE_ID = PANEL_OBJECT.slice("urn:uuid:".length);

/** One statement as the route answers it. */
interface Statement {
  curie: string;
  prefix: string;
  term: string;
  value: string;
  source: "asserted" | "extracted";
}

/** A statement of the fixture. */
function statement(curie: string, value: string, source: "asserted" | "extracted"): Statement {
  const [prefix, term] = curie.split(":") as [string, string];
  return { curie, prefix, term, value, source };
}

/** Mockup 28's statements, in record order, plus one technical statement. */
export function mockupStatements(): Statement[] {
  return [
    statement("dcterms:title", "Facade study, north pier", "asserted"),
    statement("dcterms:creator", "Mara Okafor", "asserted"),
    statement("dcterms:subject", "harbour · facade · restoration", "asserted"),
    statement(
      "dcterms:description",
      "Morning light on the restored north pier facade.",
      "asserted",
    ),
    statement("dcterms:created", "2026-10-12 14:03 (no time zone)", "extracted"),
    statement("dcterms:rights", "© 2026 Mara Okafor", "asserted"),
    statement("schema:contentLocation", "North pier, Harbour", "asserted"),
    statement("schema:copyrightYear", "2026", "asserted"),
    statement("exif:PixelXDimension", "6000", "extracted"),
  ];
}

/** The `/terms/` names a file may carry, as the metadata module's register knows them. */
const DCTERMS = new Set([
  "abstract",
  "accessRights",
  "alternative",
  "audience",
  "available",
  "bibliographicCitation",
  "conformsTo",
  "contributor",
  "coverage",
  "created",
  "creator",
  "date",
  "dateAccepted",
  "dateCopyrighted",
  "dateSubmitted",
  "description",
  "educationLevel",
  "extent",
  "format",
  "hasFormat",
  "hasPart",
  "hasVersion",
  "identifier",
  "instructionalMethod",
  "isFormatOf",
  "isPartOf",
  "isReferencedBy",
  "isReplacedBy",
  "isRequiredBy",
  "issued",
  "isVersionOf",
  "language",
  "license",
  "mediator",
  "medium",
  "modified",
  "provenance",
  "publisher",
  "references",
  "relation",
  "replaces",
  "requires",
  "rights",
  "rightsHolder",
  "source",
  "spatial",
  "subject",
  "tableOfContents",
  "temporal",
  "title",
  "type",
  "valid",
]);

/** The registry the fixture library holds. */
export const REGISTRY = [
  {
    prefix: "rico",
    iri: "https://www.ica.org/standards/RiC/ontology#",
    kind: "extension",
    registrant: "owner",
    terms: null,
  },
  {
    prefix: "harbour",
    iri: "https://example.org/harbour#",
    kind: "custom",
    registrant: "owner",
    terms: null,
  },
] as const;

/** What the fake saw and how to steer it. */
export interface MetadataApi {
  /** The body of every set the page sent. */
  readonly sets: Record<string, unknown>[];
  /** The HTTP method of every write the page sent, in order (`PUT` for a set, `POST` for a removal). */
  readonly methods: string[];
  /** The body of every removal the page sent. */
  readonly removals: Record<string, unknown>[];
  /** Every name the page asked the terms route about. */
  readonly lookups: string[];
  /** Answer the statements route with this status and code instead (an error state). */
  failStatements(status: number, code: string): void;
  /** Hold the statements route until the returned function is called (the loading state). */
  holdStatements(): () => void;
}

/** How to serve the fixture. */
export interface MetadataOptions {
  /** Start with no descriptive statement (the empty state). */
  readonly empty?: boolean;
}

/**
 * Serve the metadata routes for the fixture file, over the routes `servePanel` registered.
 *
 * @param page - The page under test.
 * @param options - Whether the record starts empty.
 * @returns What the fake saw, and ways to fail or hold the statements route.
 */
export async function serveMetadata(
  page: Page,
  options: MetadataOptions = {},
): Promise<MetadataApi> {
  let statements = options.empty ? [] : mockupStatements();
  let head = 4;
  const fail: { status?: number; code?: string } = {};
  let hold: Promise<void> | undefined;
  const api: MetadataApi = {
    sets: [],
    methods: [],
    removals: [],
    lookups: [],
    failStatements: (status, code) => Object.assign(fail, { status, code }),
    holdStatements: () => {
      let release: () => void = () => undefined;
      hold = new Promise<void>((resolve) => (release = resolve));
      return () => release();
    },
  };
  const json = (route: Route, body: unknown) => route.fulfill({ json: body });
  const refuse = (route: Route, status: number, code: string) =>
    route.fulfill({
      status,
      json: {
        code,
        message: "refused",
        retryable: code === "unavailable" || code === "rate_limited",
      },
    });
  const commit = () => {
    head += 1;
    const version = `v${head}`;
    return {
      library_id: PANEL_LIBRARY,
      object_id: PANEL_OBJECT,
      version,
      created: "2026-10-16T13:00:00Z",
    };
  };
  await page.route(/\/api\/v1\/files\/metadata\?/, async (route) => {
    await hold;
    if (fail.status) return refuse(route, fail.status, fail.code ?? "internal");
    const url = new URL(route.request().url());
    const version = url.searchParams.get("version") ?? `v${head}`;
    const body = { library_id: PANEL_LIBRARY, file_id: METADATA_FILE_ID, version, statements };
    return json(route, body);
  });
  await page.route("**/api/v1/files/metadata/statements", (route) => {
    const body = route.request().postDataJSON() as { curie: string; value: string };
    api.methods.push(route.request().method());
    api.sets.push(body);
    const [prefix, term] = body.curie.split(":") as [string, string];
    const known =
      (prefix === "dcterms" && DCTERMS.has(term)) ||
      prefix === "schema" ||
      REGISTRY.some((r) => r.prefix === prefix);
    if (!known) return refuse(route, 400, "invalid_request");
    const existing = statements.find((s) => s.curie === body.curie);
    if (existing?.value === body.value) return refuse(route, 409, "conflict");
    if (existing) existing.value = body.value;
    else statements = [...statements, statement(body.curie, body.value, "asserted")];
    return json(route, commit());
  });
  await page.route("**/api/v1/files/metadata/removals", (route) => {
    const body = route.request().postDataJSON() as { curie: string };
    api.methods.push(route.request().method());
    api.removals.push(body);
    const before = statements.length;
    statements = statements.filter((s) => s.curie !== body.curie);
    if (statements.length === before) return refuse(route, 404, "not_found");
    return json(route, commit());
  });
  await page.route(/\/api\/v1\/files\/metadata\/terms\?/, (route) => {
    const name = new URL(route.request().url()).searchParams.get("name") ?? "";
    api.lookups.push(name);
    const candidates: { curie: string; kind: string }[] = [];
    if (DCTERMS.has(name)) candidates.push({ curie: `dcterms:${name}`, kind: "dcterms" });
    candidates.push({ curie: `schema:${name}`, kind: "schema" });
    for (const r of REGISTRY) candidates.push({ curie: `${r.prefix}:${name}`, kind: r.kind });
    return json(route, { name, candidates });
  });
  await page.route(/\/api\/v1\/files\/metadata\/namespaces\?/, (route) =>
    json(route, { library_id: PANEL_LIBRARY, namespaces: REGISTRY }),
  );
  return api;
}
