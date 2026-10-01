import { readFileSync, readdirSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";
import { Hit, ResultDecodeError, ResultGroup, SearchResults } from "../src/client/results.js";
import { uint64, value } from "../src/client/values.js";
import { CoverageDegradedT } from "../src/semantic.js";
import { CoverageT } from "../src/semantic.js";
import { FiniteRelevanceT } from "../src/semantic.js";
import { HitT } from "../src/semantic.js";
import { OptionalBoolT } from "../src/semantic.js";
import { OptionalDoubleT } from "../src/semantic.js";
import { OptionalUInt32T } from "../src/semantic.js";
import { OptionalUInt64T } from "../src/semantic.js";
import { PositiveInfinityRelevanceT } from "../src/semantic.js";
import { NegativeInfinityRelevanceT } from "../src/semantic.js";
import { RelevanceKind } from "../src/semantic.js";
import { RelevanceT } from "../src/semantic.js";
import { ResultGroupT } from "../src/semantic.js";
import { ResultNodeKind } from "../src/semantic.js";
import { ResultNodeT } from "../src/semantic.js";
import { SearchResultT } from "../src/semantic.js";
import { StringEntryT } from "../src/semantic.js";
import { TotalHitsRelation } from "../src/semantic.js";
import { TotalHitsT } from "../src/semantic.js";
import { ValueEntryT } from "../src/semantic.js";
import { unpackResponse } from "../src/protocol/envelope.js";

const fixtureRoot = resolve(
  process.cwd(),
  "../../core/crates/searchkernel-dispatch/tests/fixtures/portable/responses",
);
const conformanceFixtureRoot = resolve(
  process.cwd(),
  "../../reference/result-response-conformance/fixtures",
);

function loadFixture(name: string): SearchResults {
  const bytes = readFileSync(`${fixtureRoot}/result-${name}.skrs`);
  const response = unpackResponse(new Uint8Array(bytes), "search");
  return new SearchResults(response.payload as SearchResultT);
}

const node = (group: ResultGroupT) => new ResultNodeT(ResultNodeKind.ResultGroup, group);
const searchResult = (root: ResultNodeT) =>
  new SearchResultT(root, new TotalHitsT(0n, TotalHitsRelation.Exact));

describe("ergonomic search results", () => {
  it("rejects missing canonical root and total-hits state", () => {
    expect(() => new SearchResults(
      new SearchResultT(null, new TotalHitsT(0n, TotalHitsRelation.Exact)),
    )).toThrow(/root/);
    expect(() => new SearchResults(new SearchResultT(node(new ResultGroupT()), null))).toThrow(/total_hits/);
  });

  it("projects hits, groups, facets, aggregations, coverage and continuations without losing root", () => {
    const hit = new HitT(
      "doc-1",
      new RelevanceT(RelevanceKind.FiniteRelevance, new FiniteRelevanceT(2.5)),
      [new ValueEntryT("title", value("Rust"))],
      [],
      null,
      [],
      [new ValueEntryT("blob", value(new Uint8Array([1, 2])))],
    );

    const brandBucket = new ResultGroupT(
      null,
      null,
      value("Acme"),
      [new ValueEntryT("count", value(uint64("3")))],
    );
    const facetGroup = new ResultGroupT(
      null,
      "brand",
      null,
      [],
      [node(brandBucket)],
    );
    const root = new ResultGroupT(
      null,
      null,
      null,
      [new ValueEntryT("avg_price", value(12.5))],
      [new ResultNodeT(ResultNodeKind.Hit, hit), node(facetGroup)],
      [new StringEntryT("next", "token-1")],
    );

    const result = new SearchResults(new SearchResultT(
      node(root),
      new TotalHitsT(10n, TotalHitsRelation.Exact),
      new CoverageT(
        new OptionalDoubleT(100),
        new OptionalUInt64T(10n),
        new OptionalBoolT(true),
        new OptionalUInt32T(1),
        new OptionalUInt64T(10n),
        new OptionalUInt64T(10n),
        new CoverageDegradedT(new OptionalBoolT(false)),
      ),
    ));

    expect(result.hits[0]?.id).toBe("doc-1");
    expect(result.hits[0]?.get("title")).toBe("Rust");
    expect(result.hits[0]?.matchFeatures.blob).toEqual(new Uint8Array([1, 2]));
    expect(result.groups[0]?.label).toBe("brand");
    expect(result.facets).toEqual({});
    result.projectFacet("brand", "brand");
    expect(result.facets.brand).toEqual([{ value: "Acme", count: 3n }]);
    expect(result.aggregations.avg_price).toBe(12.5);
    expect(result.totalHits).toBe(10n);
    expect(result.totalHitsRelation).toBe("exact");
    expect(result.coverage?.full).toBe(true);
    expect(result.continuations).toEqual([{ key: "next", value: "token-1" }]);
    expect(result.root).not.toBeNull();
  });
});

it("does not flatten unlabeled group wrappers by response shape", () => {
  const bucket = new ResultGroupT(null, "bucket");
  const wrapper = new ResultGroupT(null, null, null, [], [node(bucket)]);
  const root = new ResultGroupT(null, null, null, [], [node(wrapper)]);

  const result = new SearchResults(searchResult(node(root)));

  expect(result.groups).toHaveLength(1);
  expect(result.groups[0]?.label).toBeNull();
  expect(result.groups[0]?.groups[0]?.label).toBe("bucket");
});

it("fails closed for mismatched result-node unions", () => {
  const malformed = new ResultGroupT(
    null,
    null,
    null,
    [],
    [new ResultNodeT(ResultNodeKind.Hit, null)],
  );
  expect(() => new SearchResults(searchResult(node(malformed)))).toThrow(/hit node has a missing or mismatched payload/);
});

it("rejects duplicate semantic-map keys in result fields", () => {
  const malformed = new HitT(
    "doc-1",
    null,
    [new ValueEntryT("same", value(1)), new ValueEntryT("same", value(2))],
  );
  const root = new ResultGroupT(null, null, null, [], [new ResultNodeT(ResultNodeKind.Hit, malformed)]);
  expect(() => new SearchResults(searchResult(node(root)))).toThrow(/duplicate result map key/);
});

it("rejects malformed result semantic-map entries and missing Value payloads", () => {
  const cases = [
    [null as unknown as ValueEntryT],
    [new ValueEntryT("missing", null)],
  ];

  for (const fields of cases) {
    const malformed = new HitT("doc-1", null, fields);
    const root = new ResultGroupT(
      null,
      null,
      null,
      [],
      [new ResultNodeT(ResultNodeKind.Hit, malformed)],
    );
    expect(() => new SearchResults(searchResult(node(root)))).toThrow(ResultDecodeError);
  }
});


it("does not infer arbitrary labeled groups or synthesize malformed facet counts", () => {
  const exactBucket = new ResultGroupT(
    null,
    null,
    value("Acme"),
    [new ValueEntryT("count", value(uint64("3")))],
  );
  const wrongBucket = new ResultGroupT(
    null,
    null,
    value("Acme"),
    [new ValueEntryT("count", value(3))],
  );
  const arbitrary = new ResultGroupT(null, "brand", null, [], [node(exactBucket)]);
  const malformed = new ResultGroupT(null, "brand", null, [], [node(wrongBucket)]);

  const arbitraryRoot = new ResultGroupT(null, null, null, [], [node(arbitrary)]);
  const malformedRoot = new ResultGroupT(null, null, null, [], [node(malformed)]);

  const generic = new SearchResults(searchResult(node(arbitraryRoot)));
  expect(generic.facets).toEqual({});

  const wrong = new SearchResults(searchResult(node(malformedRoot)));
  wrong.projectFacet("brand", "brand");
  expect(wrong.facets).toEqual({});
});

it("does not project facets when the parent carries extra metadata", () => {
  const bucket = new ResultGroupT(
    null,
    null,
    value("Acme"),
    [new ValueEntryT("count", value(uint64("3")))],
  );
  const facet = new ResultGroupT(
    null,
    "brand",
    null,
    [],
    [node(bucket)],
    [],
    null,
    [],
    null,
    null,
    [new ValueEntryT("unexpected", value(true))],
  );
  const root = new ResultGroupT(null, null, null, [], [node(facet)]);
  const result = new SearchResults(searchResult(node(root)));

  result.projectFacet("brand", "brand");
  expect(result.facets).toEqual({});
});

it("projects named facets only when explicitly requested", () => {
  const bucket = new ResultGroupT(
    null,
    null,
    value("Acme"),
    [new ValueEntryT("count", value(uint64("3")))],
  );
  const facet = new ResultGroupT(null, "brands", null, [], [node(bucket)]);
  const root = new ResultGroupT(null, null, null, [], [node(facet)]);
  const result = new SearchResults(searchResult(node(root)));

  result.projectFacet("brands", "brands");
  expect(result.facets).toEqual({ brands: [{ value: "Acme", count: 3n }] });
});


it("projects native generic named facets with non-negative Int64 counts", () => {
  const bucket = new ResultGroupT(
    "grouping-0",
    "brands",
    value("Acme"),
    [new ValueEntryT("count", value(3))],
  );
  const facet = new ResultGroupT("brand", "brands", null, [], [node(bucket)]);
  const root = new ResultGroupT(null, null, null, [], [node(facet)]);
  const result = new SearchResults(searchResult(node(root)));

  result.projectFacet("brands", "brands", "brand");
  expect(result.facets).toEqual({ brands: [{ value: "Acme", count: 3n }] });

  const negativeBucket = new ResultGroupT(
    "grouping-0",
    "brands",
    value("Acme"),
    [new ValueEntryT("count", value(-1))],
  );
  const negativeFacet = new ResultGroupT("brand", "brands", null, [], [node(negativeBucket)]);
  const negativeRoot = new ResultGroupT(null, null, null, [], [node(negativeFacet)]);
  const negative = new SearchResults(searchResult(node(negativeRoot)));

  negative.projectFacet("brands", "brands", "brand");
  expect(negative.facets).toEqual({});
});



it("decodes timing and result issues into facade values", () => {
  const root = new ResultGroupT();
  const timingWire = {
    queryTimeSeconds: new OptionalDoubleT(0.1),
    searchTimeSeconds: new OptionalDoubleT(0.2),
    summaryFetchTimeSeconds: new OptionalDoubleT(0.3),
    metadata: [new ValueEntryT("phase", value("query"))],
  };
  const issueWire = {
    code: 7n,
    summary: "degraded",
    message: "partial",
    source: "backend",
    stackTrace: "trace",
    transient: new OptionalBoolT(true),
    metadata: [new ValueEntryT("retry", value(false))],
  };
  const raw = searchResult(node(root));
  (raw as any).timing = timingWire;
  (raw as any).errors = [issueWire];

  const result = new SearchResults(raw);

  expect(result.timing).not.toBe(timingWire);
  expect(result.timing?.queryTimeSeconds).toBe(0.1);
  expect(result.timing?.metadata).toEqual({ phase: "query" });
  expect(result.issues[0]).not.toBe(issueWire);
  expect(result.issues[0]?.code).toBe(7n);
  expect(result.issues[0]?.transient).toBe(true);
  expect(result.issues[0]?.metadata).toEqual({ retry: false });
  expect(result.errors).toBe(result.issues);
});

it("fails closed for malformed grouping continuation entries", () => {
  const malformed = new ResultGroupT(
    null,
    null,
    null,
    [],
    [],
    [new StringEntryT(null, "token")],
  );
  expect(() => new SearchResults(searchResult(node(malformed)))).toThrow(
    /result continuation has a missing key or value/,
  );
});

it("decodes finite and infinite relevance kinds consistently", () => {
  const relevance = [
    [new RelevanceT(RelevanceKind.FiniteRelevance, new FiniteRelevanceT(2.5)), 2.5],
    [new RelevanceT(RelevanceKind.PositiveInfinityRelevance, new PositiveInfinityRelevanceT()), Number.POSITIVE_INFINITY],
    [new RelevanceT(RelevanceKind.NegativeInfinityRelevance, new NegativeInfinityRelevanceT()), Number.NEGATIVE_INFINITY],
  ] as const;

  for (const [score, expected] of relevance) {
    const hit = new HitT("doc-1", score);
    const root = new ResultGroupT(null, null, null, [], [new ResultNodeT(ResultNodeKind.Hit, hit)]);
    const result = new SearchResults(searchResult(node(root)));
    expect(result.hits[0]?.relevance).toBe(expected);
  }
});

describe("shared canonical result corpus", () => {
  // SKC: result.highlighting/basic kinds=conformance runtime=shared
  it("preserves the canonical recursive tree and envelope state", () => {
    const result = loadFixture("canonical");
    if (!(result.root instanceof ResultGroup)) throw new Error("canonical root is not a group");
    const root = result.root;
    expect(root.id).toBe("root");
    expect(root.children.map((child) => child instanceof Hit ? "hit" : "group")).toEqual(["hit", "group"]);
    expect(result.hits.map((hit) => hit.id)).toEqual(["doc-1"]);
    expect(result.groups.map((group) => group.id)).toEqual(["nested"]);
    expect(result.groups[0]?.hits.map((hit) => hit.id)).toEqual(["doc-2"]);
    expect(result.aggregations).toEqual({ count: 2n });
    expect(result.groups[0]?.fields).toEqual({ count: 1n });
    expect(root.continuations).toEqual([{ key: "page", value: "token-1" }]);
    expect(result.groups[0]?.continuations).toEqual([{ key: "next", value: "token" }]);
    expect(root.limits).toEqual({ from: 0n, to: 20n });
    expect(root.metadata).toEqual({ trace_id: "r-1" });

    const hit = result.hits[0]!;
    expect(hit.relevance).toBe(4.5);
    expect(hit.highlights).toEqual([{
      field: "title",
      fragments: [{ text: "<em>First</em>", matches: [{ startByte: 0, endByte: 5 }] }],
    }]);
    expect(hit.matchFeatures).toEqual({ bm25: 2 });
    expect(hit.summaryFeatures).toEqual({ summary: "ready" });
    expect(hit.rankFeatures).toEqual({ rank: 8 });
    expect(hit.sortValues).toEqual([1n]);
    expect(result.totalHits).toBe(12n);
    expect(result.coverage?.coveragePercent).toBe(99);
    expect(result.coverage?.metadata).toEqual({ region: "west" });
    expect(result.timing?.searchTimeSeconds).toBe(0.002);
    expect(result.timing?.metadata).toEqual({ phase: "done" });
    expect(result.errors[0]?.code).toBe(503n);
    expect(result.errors[0]?.metadata).toEqual({ node: "2" });
    expect(result.trace).toEqual({ level: 3n });
    expect(result.metadata).toEqual({ request_id: "r1" });
  });

  it("preserves infinities and absent optional envelope state", () => {
    const result = loadFixture("infinite-relevance");
    expect(result.hits[1]?.relevance).toBe(Number.POSITIVE_INFINITY);
    expect(result.hits[2]?.relevance).toBe(Number.NEGATIVE_INFINITY);

    const empty = loadFixture("empty");
    expect(empty.hits).toEqual([]);
    expect(empty.groups).toEqual([]);
    expect(empty.coverage).toBeNull();
    expect(empty.timing).toBeNull();
    expect(empty.metadata).toEqual({});

    const rootHit = loadFixture("root-hit");
    expect(rootHit.hits.map((hit) => hit.id)).toEqual(["root-hit"]);
    expect(rootHit.groups).toEqual([]);

    const stateOnly = loadFixture("root-state-only");
    if (!(stateOnly.root instanceof ResultGroup)) throw new Error("state-only root is not a group");
    expect(stateOnly.continuations).toEqual([{ key: "page", value: "token" }]);
    expect(stateOnly.root.limits).toEqual({ from: 1n, to: 2n });
    expect(stateOnly.root.metadata).toEqual({ state: "present" });
  });

  it("preserves sparse and mixed tensors from the shared response fixture", () => {
    const result = loadFixture("sparse-mixed-tensors");
    if (!(result.root instanceof ResultGroup)) throw new Error("tensor fixture root is not a group");
    expect(result.hits[0]?.get("sparse")).toEqual({
      $searchkernel_tensor: {
        typeSpec: "tensor<float>(label{},region{})",
        sparse: [
          {
            address: [
              { dimension: "label", label: "a" },
              { dimension: "region", label: "east" },
            ],
            value: 0.5,
          },
          {
            address: [
              { dimension: "label", label: "b" },
              { dimension: "region", label: "west" },
            ],
            value: -0.25,
          },
        ],
      },
    });
    expect(result.root.value).toEqual({
      $searchkernel_tensor: {
        typeSpec: "tensor<float>(label{},region{},x[2])",
        mixed: [
          {
            address: [
              { dimension: "label", label: "b" },
              { dimension: "region", label: "west" },
            ],
            values: [0.25, 0.75],
          },
          {
            address: [
              { dimension: "label", label: "c" },
              { dimension: "region", label: "north" },
            ],
            values: [1.5, -2.0],
          },
        ],
      },
    });
  });

  it("uses the exact Rust facet shape for every shared fixture", () => {
    const valid = loadFixture("facet-valid");
    valid.projectFacet("brand", "brand");
    expect(valid.facets).toEqual({ brand: [{ value: "Acme", count: 3n }] });

    const named = loadFixture("facet-valid-named");
    named.projectFacet("brands", "brands", "brand");
    expect(named.facets).toEqual({ brands: [{ value: "Acme", count: 3n }] });

    for (const filename of readdirSync(fixtureRoot).filter((item) => item.startsWith("result-facet-"))) {
      if (filename === "result-facet-valid.skrs" || filename === "result-facet-valid-named.skrs") continue;
      const result = loadFixture(filename.slice("result-".length, -".skrs".length));
      result.projectFacet("brand", "brand");
      expect(result.facets, filename).toEqual({});
    }
  });
  it.each([
    "invalid-tensor-dense-nan.skrs",
    "invalid-tensor-sparse-positive-infinity.skrs",
    "invalid-tensor-mixed-negative-infinity.skrs",
  ])("rejects forbidden tensor cells in %s", (filename) => {
    expect(() => {
      const bytes = readFileSync(`${conformanceFixtureRoot}/${filename}`);
      const response = unpackResponse(new Uint8Array(bytes), "search");
      return new SearchResults(response.payload as SearchResultT);
    }).toThrow(/tensor cells must be finite/);
  });


});
