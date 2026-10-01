import { expect, it } from "vitest";
import { readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { unpackResponse } from "../src/protocol/envelope.js";
import { Hit, ResultGroup, SearchResults } from "../src/client/results.js";
import { SearchResultT } from "../src/semantic.js";

function value(input: unknown): unknown {
  if (input == null || typeof input === "boolean" || typeof input === "string") return input;
  if (typeof input === "bigint") return input.toString();
  if (typeof input === "number") {
    if (Number.isNaN(input)) return "nan";
    if (input === Number.POSITIVE_INFINITY) return "+inf";
    if (input === Number.NEGATIVE_INFINITY) return "-inf";
    return input;
  }
  if (input instanceof Uint8Array) return Array.from(input, (item) => item.toString());
  if (Array.isArray(input)) return input.map(value);
  if (typeof input === "object") {
    const source = input as Record<string, unknown>;
    if ("$searchkernel_tensor" in source) {
      const tensor = source.$searchkernel_tensor as Record<string, unknown>;
      const normalized: Record<string, unknown> = {
        type_spec: tensor.typeSpec ?? tensor.type_spec ?? null,
      };
      for (const key of ["dense", "sparse", "mixed"]) {
        if (key in tensor) normalized[key] = value(tensor[key]);
      }
      return { $searchkernel_tensor: normalized };
    }
    const normalized: Record<string, unknown> = {};
    for (const key of Object.keys(source).sort()) normalized[key] = value(source[key]);
    return normalized;
  }
  throw new TypeError("unsupported trace value: " + typeof input);
}

function nodeTrace(node: Hit | ResultGroup | null): unknown {
  if (node == null) return null;
  if (node instanceof Hit) {
    return {
      kind: "hit",
      id: node.id,
      relevance: value(node.relevance),
      fields: value(node.fields),
      highlights: node.highlights.map((highlight) => ({
        field: highlight.field,
        fragments: highlight.fragments.map((fragment) => ({
          text: fragment.text,
          matches: fragment.matches.map((match) => ({
            start_byte: match.startByte.toString(),
            end_byte: match.endByte.toString(),
          })),
        })),
      })),
      source: node.source,
      types: [...node.types],
      match_features: value(node.matchFeatures),
      summary_features: value(node.summaryFeatures),
      rank_features: value(node.rankFeatures),
      sort_values: value(node.sortValues),
    };
  }
  return {
    kind: "group",
    id: node.id,
    label: node.label,
    value: value(node.value),
    fields: value(node.fields),
    children: node.children.map((child) => nodeTrace(child)),
    continuations: node.continuations.map((item) => ({ key: item.key, value: item.value })),
    source: node.source,
    types: [...node.types],
    relevance: value(node.relevance),
    limits: node.limits == null
      ? null
      : { from: value(node.limits.from), to: value(node.limits.to) },
    metadata: value(node.metadata),
  };
}

function coverageTrace(coverage: SearchResults["coverage"]): unknown {
  if (coverage == null) return null;
  return {
    coverage_percent: value(coverage.coveragePercent),
    documents: value(coverage.documents),
    full: coverage.full,
    nodes: coverage.nodes == null ? null : coverage.nodes.toString(),
    results: value(coverage.results),
    results_full: value(coverage.resultsFull),
    degraded: coverage.degraded == null
      ? null
      : {
          match_phase: coverage.degraded.matchPhase,
          timeout: coverage.degraded.timeout,
          adaptive_timeout: coverage.degraded.adaptiveTimeout,
          ann_timeout: coverage.degraded.annTimeout,
          non_ideal_state: coverage.degraded.nonIdealState,
          metadata: value(coverage.degraded.metadata),
        },
    metadata: value(coverage.metadata),
  };
}

function timingTrace(timing: SearchResults["timing"]): unknown {
  if (timing == null) return null;
  return {
    query_time_seconds: value(timing.queryTimeSeconds),
    search_time_seconds: value(timing.searchTimeSeconds),
    summary_fetch_time_seconds: value(timing.summaryFetchTimeSeconds),
    metadata: value(timing.metadata),
  };
}

function trace(result: SearchResults): unknown {
  return {
    root: nodeTrace(result.root),
    total_hits: { value: result.totalHits.toString(), relation: result.totalHitsRelation },
    coverage: coverageTrace(result.coverage),
    timing: timingTrace(result.timing),
    errors: result.errors.map((issue) => ({
      code: issue.code.toString(),
      summary: issue.summary,
      message: issue.message,
      source: issue.source,
      stack_trace: issue.stackTrace,
      transient: issue.transient,
      metadata: value(issue.metadata),
    })),
    trace: value(result.trace),
    metadata: value(result.metadata),
  };
}

it.skipIf(!process.env.SEARCHKERNEL_RESULT_CASES)("decodes every canonical SearchResult response byte fixture", () => {
  const cases = JSON.parse(readFileSync(process.env.SEARCHKERNEL_RESULT_CASES!, "utf8"));
  const caseDirectory = process.env.SEARCHKERNEL_RESULT_CASE_DIR!;
  const traces: Record<string, unknown> = {};
  for (const scenario of cases.valid as Array<{ id: string; file: string }>) {
    const bytes = new Uint8Array(readFileSync(join(caseDirectory, scenario.file)));
    const decoded = unpackResponse(bytes, "search");
    expect(decoded.payload).toBeInstanceOf(SearchResultT);
    const result = new SearchResults(decoded.payload as SearchResultT);
    expect(result.root).not.toBeNull();
    traces[scenario.id] = trace(result);
  }

  const outcomes: Record<string, string> = {};
  for (const scenario of cases.malformed as Array<{ id: string; file: string }>) {
    const bytes = new Uint8Array(readFileSync(join(process.env.SEARCHKERNEL_RESULT_FIXTURE_DIR!, scenario.file)));
    expect(() => {
      const decoded = unpackResponse(bytes, "search");
      new SearchResults(decoded.payload as SearchResultT);
    }, scenario.id).toThrow();
    outcomes[scenario.id] = "client";
  }
  traces.malformed_outcomes = outcomes;
  writeFileSync(join(process.env.SEARCHKERNEL_RESULT_TRACE_DIR!, "typescript.json"), JSON.stringify(traces));
});
