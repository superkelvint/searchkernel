/** Representation boundary for the pinned contract in compatibility.json. */
import compatibility from "./compatibility.json" with { type: "json" };
export type SearchKernelErrorCode = "invalid_request" | "unsupported" | "unknown_index" |
  "stale_target" | "unavailable" | "timeout" | "overloaded" | "backend_failure" | "invalid_response";

const messages: Record<SearchKernelErrorCode, string> = {
  invalid_request: "Invalid search request", unsupported: "Unsupported search capability",
  unknown_index: "Unknown search index", stale_target: "Search index configuration is stale",
  unavailable: "Search service unavailable", timeout: "Search request timed out",
  overloaded: "Search service is at capacity", backend_failure: "Search execution failed",
  invalid_response: "Invalid search service response",
};

export class SearchKernelInstantSearchError extends Error {
  readonly name = "SearchKernelInstantSearchError";
  constructor(readonly code: SearchKernelErrorCode, readonly requestIndex?: number) {
    super(messages[code]);
  }
}

export interface SearchKernelInstantSearchOptions {
  /** URL of the separately configured public search listener, ending in /search. */
  endpoint: string;
  timeoutMs?: number;
  fetch?: typeof globalThis.fetch;
}

export interface InstantSearchRequest {
  indexName: string;
  params?: Record<string, unknown>;
}

type Filter = { op: "and" | "or"; clauses: Filter[] } |
  { op: "eq" | "ne" | "lt" | "lte" | "gt" | "gte"; field: string; value: string | number };

interface EndpointRequest {
  endpoint: string;
  query: string;
  limit: number;
  offset: number;
  facets: string[];
  facetLimit: number;
  filters: Filter[];
  fields?: string[];
  highlights?: string[];
  snippets: string[];
}

interface Fragment { text: string; matches: { startByte: number; endByte: number }[] }
interface EndpointResult {
  endpoint: string;
  query: string;
  totalHits: number;
  exact: boolean;
  processingTimeMs: number;
  facets: Record<string, Record<string, number>>;
  facetsStats?: Record<string, { min: number; max: number }>;
  exhaustiveFacetsCount: boolean;
  hits: { id: string; fields: Record<string, unknown>; highlights: Record<string, Fragment[]> }[];
}

const allowed = new Set(Object.keys(compatibility.parameters));

function fail(code: SearchKernelErrorCode = "invalid_request"): never {
  throw new SearchKernelInstantSearchError(code);
}
function strings(input: unknown, max = 32): string[] {
  const list = typeof input === "string" ? [input] : input;
  if (!Array.isArray(list) || list.length > max || list.some(v => typeof v !== "string" || v.length > 1024)) fail();
  return list as string[];
}
function integer(input: unknown, fallback: number, max: number): number {
  const value = input === undefined ? fallback : input;
  if (typeof value !== "number" || !Number.isSafeInteger(value) || value < 0 || value > max) fail();
  return value;
}

function filters(input: unknown, parse: (item: string) => Filter): Filter[] {
  if (input === undefined) return [];
  const list = typeof input === "string" ? [input] : input;
  if (!Array.isArray(list) || list.length > 32) fail();
  return list.map(item => {
    if (Array.isArray(item)) {
      const clauses = strings(item).map(parse);
      if (clauses.length === 0) fail();
      return { op: "or", clauses };
    }
    if (typeof item !== "string" || item.length > 1024) fail();
    return parse(item);
  });
}

function facetFilter(input: string): Filter {
  const colon = input.indexOf(":");
  if (colon <= 0) fail();
  let value = input.slice(colon + 1);
  const op = value.startsWith("-") ? "ne" : "eq";
  if (op === "ne") value = value.slice(1);
  else if (value.startsWith("\\-")) value = value.slice(1);
  return { field: input.slice(0, colon), op, value };
}

function numericFilter(input: string): Filter {
  const match = /^([A-Za-z_][A-Za-z_0-9.]*)\s*(<=|>=|!=|=|<|>)\s*(-?(?:\d+(?:\.\d*)?|\.\d+)(?:[eE][+-]?\d+)?)$/.exec(input);
  if (!match) fail();
  const value = Number(match[3]);
  if (!Number.isFinite(value)) fail();
  const ops = { "=": "eq", "!=": "ne", "<": "lt", "<=": "lte", ">": "gt", ">=": "gte" } as const;
  return { field: match[1], op: ops[match[2] as keyof typeof ops], value };
}

function translate(request: InstantSearchRequest): EndpointRequest {
  if (!request || typeof request.indexName !== "string" || !/^[A-Za-z0-9_-]{1,64}$/.test(request.indexName)) fail();
  if (Object.keys(request).some(key => key !== "indexName" && key !== "params")) fail("unsupported");
  const params = request.params === undefined ? {} : request.params;
  if (!params || typeof params !== "object" || Array.isArray(params)) fail();
  for (const key of Object.keys(params)) if (!allowed.has(key)) fail("unsupported");
  for (const key of ["analytics", "clickAnalytics"]) {
    if (params[key] !== undefined && params[key] !== false) fail("unsupported");
  }
  // The pinned Hits connector requests these placeholders and performs HTML
  // escaping itself. Custom markup / escapeHTML:false is outside this profile.
  if ((params.highlightPreTag !== undefined && params.highlightPreTag !== "__ais-highlight__") ||
      (params.highlightPostTag !== undefined && params.highlightPostTag !== "__/ais-highlight__")) fail("unsupported");
  const query = params.query === undefined ? "" : params.query;
  if (typeof query !== "string" || new TextEncoder().encode(query).length > 4096 || query.includes("\0")) fail();
  const limit = integer(params.hitsPerPage, 20, 100);
  const page = integer(params.page, 0, 10000);
  const offset = page * limit;
  if (offset > 10000 || (limit === 0 && page !== 0)) fail();
  const snippets = params.attributesToSnippet === undefined ? [] : strings(params.attributesToSnippet);
  // Juniper owns excerpt selection. Algolia word-count directives cannot be
  // represented by the canonical teaser request and must not be approximated.
  if (snippets.some(field => field.includes(":"))) fail("unsupported");
  const facetLimit = integer(params.maxValuesPerFacet, 100, 100);
  if (facetLimit === 0) fail();
  return {
    endpoint: request.indexName, query, limit, offset,
    facets: params.facets === undefined ? [] : strings(params.facets),
    facetLimit,
    filters: [...filters(params.facetFilters, facetFilter), ...filters(params.numericFilters, numericFilter)],
    fields: params.attributesToRetrieve === undefined ? undefined : strings(params.attributesToRetrieve),
    highlights: params.attributesToHighlight === undefined ? undefined : strings(params.attributesToHighlight),
    snippets,
  };
}

function highlighted(fragments: Fragment[]) {
  if (!Array.isArray(fragments) || fragments.length > 64) fail("invalid_response");
  let matched = false;
  const matchedWords = new Set<string>();
  const value = fragments.map(fragment => {
    const bytes = new TextEncoder().encode(fragment.text);
    const decoder = new TextDecoder("utf-8", { fatal: true });
    let offset = 0;
    let result = "";
    for (const span of fragment.matches) {
      if (!Number.isInteger(span.startByte) || !Number.isInteger(span.endByte) || span.startByte < offset ||
          span.endByte <= span.startByte || span.endByte > bytes.length) fail("invalid_response");
      result += decoder.decode(bytes.slice(offset, span.startByte));
      const word = decoder.decode(bytes.slice(span.startByte, span.endByte));
      matchedWords.add(word);
      result += "__ais-highlight__" + word + "__/ais-highlight__";
      offset = span.endByte;
      matched = true;
    }
    return result + decoder.decode(bytes.slice(offset));
  }).join(" … ");
  // Spans prove a match, but not that every query term matched this attribute.
  return { value, matchLevel: matched ? "partial" : "none", matchedWords: [...matchedWords] };
}

/** One projection boundary; canonical document fields never become Algolia models. */
function project(result: EndpointResult, request: InstantSearchRequest, translated: EndpointRequest) {
  if (result.endpoint !== request.indexName || result.query !== translated.query ||
      !Number.isSafeInteger(result.totalHits) || result.totalHits < 0 || !Array.isArray(result.hits) ||
      result.hits.length > translated.limit || !Number.isFinite(result.processingTimeMs) || result.processingTimeMs < 0 ||
      typeof result.exact !== "boolean" || typeof result.exhaustiveFacetsCount !== "boolean" ||
      !result.facets || typeof result.facets !== "object" || Array.isArray(result.facets)) fail("invalid_response");
  for (const counts of Object.values(result.facets)) {
    if (!counts || typeof counts !== "object" || Array.isArray(counts) ||
        Object.values(counts).some(count => !Number.isSafeInteger(count) || count < 0)) fail("invalid_response");
  }
  const hits = result.hits.map(hit => {
    if (typeof hit.id !== "string" || !hit.fields || Array.isArray(hit.fields) || typeof hit.fields !== "object" ||
        !hit.highlights || Array.isArray(hit.highlights) || typeof hit.highlights !== "object") fail("invalid_response");
    const highlightResult: Record<string, ReturnType<typeof highlighted>> = Object.create(null);
    const snippetResult: Record<string, ReturnType<typeof highlighted>> = Object.create(null);
    for (const [field, fragments] of Object.entries(hit.highlights)) {
      const value = highlighted(fragments);
      if (translated.snippets.includes(field)) snippetResult[field] = value;
      if (translated.highlights === undefined || translated.highlights.includes("*") || translated.highlights.includes(field)) {
        highlightResult[field] = value;
      }
    }
    return { ...hit.fields, objectID: hit.id, _highlightResult: highlightResult, _snippetResult: snippetResult };
  });
  return {
    hits, nbHits: result.totalHits,
    nbPages: translated.limit === 0 ? 0 : Math.ceil(result.totalHits / translated.limit),
    page: integer(request.params?.page, 0, 10000), hitsPerPage: translated.limit,
    query: translated.query, index: request.indexName,
    processingTimeMS: result.processingTimeMs,
    exhaustiveNbHits: result.exact, exhaustiveFacetsCount: result.exhaustiveFacetsCount,
    facets: result.facets, facets_stats: result.facetsStats ?? {},
    params: new URLSearchParams(Object.entries(request.params ?? {}).map(([key, value]) =>
      [key, typeof value === "object" ? JSON.stringify(value) : String(value)])).toString(),
  };
}

/** Stateless, browser-capable search client. A failed batch rejects as a whole. */
export function createSearchKernelInstantSearchClient(options: SearchKernelInstantSearchOptions) {
  let endpoint: URL;
  try { endpoint = new URL(options.endpoint); } catch { fail(); }
  if (!["http:", "https:"].includes(endpoint.protocol) || endpoint.username || endpoint.password) fail();
  const timeoutMs = integer(options.timeoutMs, 5000, 30000);
  if (timeoutMs === 0) fail();
  const fetch = options.fetch ?? globalThis.fetch.bind(globalThis);
  return {
    // The pinned helper calls this optional method for consumer identification.
    // It does not change query semantics or send analytics.
    addAlgoliaAgent(_agent: string, _version?: string) {},
    async searchForFacetValues(_requests: unknown) { fail("unsupported"); },
    async getRecommendations(_requests: unknown) { fail("unsupported"); },
    async search(requests: readonly InstantSearchRequest[], requestOptions?: unknown) {
      if (requestOptions !== undefined) fail("unsupported");
      if (!Array.isArray(requests) || requests.length > 16) fail();
      const translated = requests.map((request, index) => {
        try { return translate(request); } catch (error) {
          if (error instanceof SearchKernelInstantSearchError) throw new SearchKernelInstantSearchError(error.code, index);
          throw error;
        }
      });
      if (requests.length === 0) return { results: [] };
      const controller = new AbortController();
      const timer = setTimeout(() => controller.abort(), timeoutMs);
      let responseReceived = false;
      try {
        const response = await fetch(endpoint, {
          method: "POST", headers: { "Content-Type": "application/json" },
          body: JSON.stringify({ searches: translated }), signal: controller.signal,
        });
        responseReceived = true;
        if (!response.ok) {
          const body = await response.json().catch(() => null) as { error?: { code?: SearchKernelErrorCode; requestIndex?: number } } | null;
          const code = body?.error?.code;
          if (code && Object.hasOwn(messages, code)) throw new SearchKernelInstantSearchError(code, typeof body?.error?.requestIndex === "number" &&
            Number.isInteger(body.error.requestIndex) && body.error.requestIndex >= 0 ? body.error.requestIndex : undefined);
          fail(response.status === 503 ? "unavailable" : "backend_failure");
        }
        const body = await response.json() as { results?: EndpointResult[] };
        if (!Array.isArray(body.results) || body.results.length !== requests.length) fail("invalid_response");
        return { results: body.results.map((result, index) => project(result, requests[index], translated[index])) };
      } catch (error) {
        if (error instanceof SearchKernelInstantSearchError) throw error;
        fail(controller.signal.aborted ? "timeout" : responseReceived ? "invalid_response" : "unavailable");
      } finally { clearTimeout(timer); }
    },
  };
}
