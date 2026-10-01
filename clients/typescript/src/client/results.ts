import { CoverageDegradedT } from "../semantic.js";
import { CoverageT } from "../semantic.js";
import { FiniteRelevanceT } from "../semantic.js";
import { HitT } from "../semantic.js";
import { RelevanceKind } from "../semantic.js";
import { NegativeInfinityRelevanceT } from "../semantic.js";
import { PositiveInfinityRelevanceT } from "../semantic.js";
import { RelevanceT } from "../semantic.js";
import { ResultGroupT } from "../semantic.js";
import { ResultNodeKind } from "../semantic.js";
import { ResultNodeT } from "../semantic.js";
import { SearchResultErrorT } from "../semantic.js";
import { SearchResultT } from "../semantic.js";
import { SearchTimingT } from "../semantic.js";
import { TotalHitsRelation } from "../semantic.js";
import { ValueEntryT } from "../semantic.js";
import { ValueKind } from "../semantic.js";
import { fromValue } from "./values.js";

export class ResultDecodeError extends Error {
  override name = "ResultDecodeError";
}

function text(value: string | Uint8Array | null | undefined): string | null {
  if (value == null) return null;
  if (typeof value === "string") return value;
  return new TextDecoder().decode(value);
}

function entries(
  values: readonly (ValueEntryT | null)[] | null | undefined,
): Record<string, unknown> {
  const decoded: Record<string, unknown> = {};
  const seen = new Set<string>();
  for (const entry of values ?? []) {
    if (entry === null) throw new ResultDecodeError("malformed result map entry");
    const key = text(entry.key);
    if (key === null) throw new ResultDecodeError("result map entry has no key");
    if (seen.has(key)) throw new ResultDecodeError(`duplicate result map key: ${key}`);
    if (entry.value === null) {
      throw new ResultDecodeError(`result map entry has no Value payload: ${key}`);
    }
    seen.add(key);
    decoded[key] = fromValue(entry.value);
  }
  return decoded;
}

function optionalValue<T>(value: { value: T } | null | undefined): T | null {
  return value?.value ?? null;
}

function relevanceValue(relevance: RelevanceT | null | undefined): number | null {
  if (!relevance || relevance.valueType === RelevanceKind.NONE) return null;
  switch (relevance.valueType) {
    case RelevanceKind.FiniteRelevance:
      if (!(relevance.value instanceof FiniteRelevanceT)) {
        throw new ResultDecodeError("finite relevance has a missing or mismatched payload");
      }
      return relevance.value.value;
    case RelevanceKind.PositiveInfinityRelevance:
      if (!(relevance.value instanceof PositiveInfinityRelevanceT)) {
        throw new ResultDecodeError("positive-infinity relevance has a missing or mismatched payload");
      }
      return Number.POSITIVE_INFINITY;
    case RelevanceKind.NegativeInfinityRelevance:
      if (!(relevance.value instanceof NegativeInfinityRelevanceT)) {
        throw new ResultDecodeError("negative-infinity relevance has a missing or mismatched payload");
      }
      return Number.NEGATIVE_INFINITY;
    default:
      throw new ResultDecodeError(`unsupported relevance kind: ${relevance.valueType}`);
  }
}

export class CoverageDegraded {
  readonly matchPhase: boolean | null;
  readonly timeout: boolean | null;
  readonly adaptiveTimeout: boolean | null;
  readonly annTimeout: boolean | null;
  readonly nonIdealState: boolean | null;
  readonly metadata: Record<string, unknown>;

  constructor(node: CoverageDegradedT) {
    this.matchPhase = optionalValue(node.matchPhase);
    this.timeout = optionalValue(node.timeout);
    this.adaptiveTimeout = optionalValue(node.adaptiveTimeout);
    this.annTimeout = optionalValue(node.annTimeout);
    this.nonIdealState = optionalValue(node.nonIdealState);
    this.metadata = entries(node.metadata);
  }
}

export class Coverage {
  readonly coveragePercent: number | null;
  readonly documents: bigint | null;
  readonly full: boolean | null;
  readonly nodes: number | null;
  readonly results: bigint | null;
  readonly resultsFull: bigint | null;
  readonly degraded: CoverageDegraded | null;
  readonly metadata: Record<string, unknown>;

  constructor(node: CoverageT) {
    this.coveragePercent = optionalValue(node.coveragePercent);
    this.documents = optionalValue(node.documents);
    this.full = optionalValue(node.full);
    this.nodes = optionalValue(node.nodes);
    this.results = optionalValue(node.results);
    this.resultsFull = optionalValue(node.resultsFull);
    this.degraded = node.degraded ? new CoverageDegraded(node.degraded) : null;
    this.metadata = entries(node.metadata);
  }
}

export class SearchTiming {
  readonly queryTimeSeconds: number | null;
  readonly searchTimeSeconds: number | null;
  readonly summaryFetchTimeSeconds: number | null;
  readonly metadata: Record<string, unknown>;

  constructor(node: SearchTimingT) {
    this.queryTimeSeconds = optionalValue(node.queryTimeSeconds);
    this.searchTimeSeconds = optionalValue(node.searchTimeSeconds);
    this.summaryFetchTimeSeconds = optionalValue(node.summaryFetchTimeSeconds);
    this.metadata = entries(node.metadata);
  }
}

export class ResultIssue {
  readonly code: bigint;
  readonly summary: string | null;
  readonly message: string | null;
  readonly source: string | null;
  readonly stackTrace: string | null;
  readonly transient: boolean | null;
  readonly metadata: Record<string, unknown>;

  constructor(node: SearchResultErrorT) {
    this.code = node.code;
    this.summary = text(node.summary);
    this.message = text(node.message);
    this.source = text(node.source);
    this.stackTrace = text(node.stackTrace);
    this.transient = optionalValue(node.transient);
    this.metadata = entries(node.metadata);
  }
}

export class Hit {
  readonly id: string;
  readonly relevance: number | null;
  readonly score: number | null;
  readonly fields: Record<string, unknown>;
  readonly highlights: readonly {
    readonly field: string;
    readonly fragments: readonly {
      readonly text: string;
      readonly matches: readonly { readonly startByte: number; readonly endByte: number }[];
    }[];
  }[];
  readonly source: string | null;
  readonly types: readonly string[];
  readonly matchFeatures: Record<string, unknown>;
  readonly summaryFeatures: Record<string, unknown>;
  readonly rankFeatures: Record<string, unknown>;
  readonly features: Record<string, unknown>;
  readonly sortValues: readonly unknown[];

  constructor(node: HitT) {
    this.id = text(node.id) ?? "";
    this.relevance = relevanceValue(node.relevance);
    this.score = this.relevance;
    this.fields = entries(node.fields);
    this.highlights = (node.highlights ?? []).map((highlight) => ({
      field: text(highlight.field) ?? "",
      fragments: (highlight.fragments ?? []).map((fragment) => ({
        text: text(fragment.text) ?? "",
        matches: (fragment.matches ?? []).map((match) => ({
          startByte: match.startByte,
          endByte: match.endByte,
        })),
      })),
    }));
    this.source = text(node.source);
    this.types = (node.types ?? []).map((item) => text(item) ?? "");
    this.matchFeatures = entries(node.matchFeatures);
    this.summaryFeatures = entries(node.summaryFeatures);
    this.rankFeatures = entries(node.rankFeatures);
    this.features = this.matchFeatures;
    this.sortValues = (node.sortValues ?? []).map(fromValue);
  }

  get(name: string): unknown {
    return this.fields[name];
  }
}

export type ResultChild = Hit | ResultGroup;

export interface ResultContinuation {
  readonly key: string;
  readonly value: string;
}

export interface ResultLimits {
  readonly from: unknown;
  readonly to: unknown;
}

export class ResultGroup {
  readonly raw: ResultGroupT;
  readonly id: string | null;
  readonly label: string | null;
  readonly value: unknown;
  readonly fields: Record<string, unknown>;
  readonly children: readonly ResultChild[];
  readonly groups: readonly ResultGroup[];
  readonly hits: readonly Hit[];
  readonly continuations: readonly ResultContinuation[];
  readonly source: string | null;
  readonly types: readonly string[];
  readonly relevance: number | null;
  readonly limits: ResultLimits | null;
  readonly metadata: Record<string, unknown>;

  constructor(node: ResultGroupT) {
    this.raw = node;
    this.id = text(node.id);
    this.label = text(node.label);
    this.value = fromValue(node.value);
    this.fields = entries(node.fields);
    this.children = (node.children ?? []).map(wrapNode);
    this.groups = this.children.filter((child): child is ResultGroup => child instanceof ResultGroup);
    this.hits = this.children.filter((child): child is Hit => child instanceof Hit);
    this.continuations = (node.continuations ?? []).map((entry) => {
      const key = text(entry.key);
      const value = text(entry.value);
      if (key === null || value === null) {
        throw new ResultDecodeError("result continuation has a missing key or value");
      }
      return { key, value };
    });
    this.source = text(node.source);
    this.types = (node.types ?? []).map((item) => text(item) ?? "");
    this.relevance = relevanceValue(node.relevance);
    this.limits = node.limits == null
      ? null
      : { from: fromValue(node.limits.from), to: fromValue(node.limits.to) };
    this.metadata = entries(node.metadata);
  }

  get(name: string): unknown {
    return this.fields[name];
  }
}

function wrapNode(node: ResultNodeT): ResultChild {
  if (node.nodeType === ResultNodeKind.ResultGroup) {
    if (!(node.node instanceof ResultGroupT)) {
      throw new ResultDecodeError("result-group node has a missing or mismatched payload");
    }
    return new ResultGroup(node.node);
  }
  if (node.nodeType === ResultNodeKind.Hit) {
    if (!(node.node instanceof HitT)) {
      throw new ResultDecodeError("hit node has a missing or mismatched payload");
    }
    return new Hit(node.node);
  }
  throw new ResultDecodeError(`unsupported result node kind: ${node.nodeType}`);
}

export type TotalHitsRelationName = "unspecified" | "exact" | "lower_bound" | "approximate" | "unknown";

function totalHitsRelationName(relation: TotalHitsRelation | undefined): TotalHitsRelationName {
  switch (relation) {
    case TotalHitsRelation.Exact: return "exact";
    case TotalHitsRelation.LowerBound: return "lower_bound";
    case TotalHitsRelation.Approximate: return "approximate";
    case TotalHitsRelation.Unknown: return "unknown";
    default: return "unspecified";
  }
}

export interface FacetBucket {
  value: unknown;
  count: unknown;
}

export class SearchResults {
  readonly raw: SearchResultT;
  readonly root: ResultChild | null;
  readonly hits: readonly Hit[];
  readonly groups: readonly ResultGroup[];
  readonly totalHits: bigint;
  readonly totalHitsRelation: TotalHitsRelationName;
  readonly coverage: Coverage | null;
  readonly timing: SearchTiming | null;
  readonly issues: readonly ResultIssue[];
  readonly errors: readonly ResultIssue[];
  readonly aggregations: Record<string, unknown>;
  readonly continuations: readonly ResultContinuation[];
  readonly metadata: Record<string, unknown>;
  readonly trace: unknown;
  readonly facets: Record<string, FacetBucket[]>;

  constructor(result: SearchResultT) {
    this.raw = result;
    if (result.root == null) throw new ResultDecodeError("missing SearchResult.root");
    this.root = wrapNode(result.root);
    if (result.totalHits == null) throw new ResultDecodeError("missing SearchResult.total_hits");

    const rootChildren = this.root instanceof ResultGroup ? this.root.children : [];
    this.hits = this.root instanceof Hit
      ? [this.root]
      : rootChildren.filter((child): child is Hit => child instanceof Hit);

    // Convenience groups are exactly the direct ResultGroup children of
    // the canonical root. Do not infer wrapper semantics from response shape;
    // request-derived conveniences (such as facets) are projected separately
    // by the builder that knows the request.
    this.groups = rootChildren.filter((child): child is ResultGroup => child instanceof ResultGroup);

    this.totalHits = result.totalHits.value;
    this.totalHitsRelation = totalHitsRelationName(result.totalHits.relation);
    this.coverage = result.coverage ? new Coverage(result.coverage) : null;
    this.timing = result.timing ? new SearchTiming(result.timing) : null;
    this.issues = (result.errors ?? []).map((issue) => new ResultIssue(issue));
    this.errors = this.issues;
    this.aggregations = this.root instanceof ResultGroup ? { ...this.root.fields } : {};
    this.continuations = this.root instanceof ResultGroup ? [...this.root.continuations] : [];
    this.metadata = entries(result.metadata);
    this.trace = fromValue(result.trace);

    this.facets = {};
  }

  /**
   * Project the exact canonical facet result shape for a facet the caller requested.
   *
   * Generic grouping results are never inferred to be facets. Optimized facet
   * buckets carry UInt64 counts, while the native generic grouping path may
   * expose count() as a non-negative Int64. Both shapes remain request-gated.
   */
  projectFacet(expectedLabel: string, resultName: string, expectedField = expectedLabel): void {
    const candidates = this.groups.filter((group) =>
      (group.id === expectedField || group.id == null)
      && group.label === expectedLabel
      && group.value == null
      && (group.raw.fields ?? []).length === 0
      && group.continuations.length === 0
      && group.source == null
      && group.types.length === 0
      && group.relevance == null
      && group.limits == null
      && Object.keys(group.metadata).length === 0
      && group.groups.length === group.children.length
    );
    if (candidates.length !== 1) return;

    const buckets: FacetBucket[] = [];
    for (const child of candidates[0]!.groups) {
      const optimizedChild = child.id == null && child.label == null;
      const genericChild = child.id === "grouping-0" && child.label === expectedLabel;
      if (!optimizedChild && !genericChild) return;
      if (child.children.length !== 0 || child.continuations.length !== 0) return;
      if (child.source != null || child.types.length !== 0 || child.relevance != null) return;
      if (child.limits != null || Object.keys(child.metadata).length !== 0) return;
      const fields = child.raw.fields ?? [];
      if (fields.length !== 1) return;
      const count = fields[0]!;
      if (text(count.key) !== "count" || count.value == null) return;

      const acceptedCountKinds = genericChild
        ? [ValueKind.Int64Value, ValueKind.UInt64Value]
        : [ValueKind.UInt64Value];
      if (!acceptedCountKinds.includes(count.value.kindType)) return;

      const countValue = fromValue(count.value);
      if (genericChild && (typeof countValue !== "bigint" || countValue < 0n)) return;
      buckets.push({ value: child.value, count: countValue });
    }

    this.facets[resultName] = buckets;
  }
}
