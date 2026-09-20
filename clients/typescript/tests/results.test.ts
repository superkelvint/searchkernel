import { describe, expect, it } from "vitest";
import { SearchResults } from "../src/client/results.js";
import { value } from "../src/client/values.js";
import { CoverageDegradedT } from "../src/generated/flatbuffers/search-kernel/v2/coverage-degraded.js";
import { CoverageT } from "../src/generated/flatbuffers/search-kernel/v2/coverage.js";
import { FiniteRelevanceT } from "../src/generated/flatbuffers/search-kernel/v2/finite-relevance.js";
import { HitT } from "../src/generated/flatbuffers/search-kernel/v2/hit.js";
import { OptionalBoolT } from "../src/generated/flatbuffers/search-kernel/v2/optional-bool.js";
import { OptionalDoubleT } from "../src/generated/flatbuffers/search-kernel/v2/optional-double.js";
import { OptionalUInt32T } from "../src/generated/flatbuffers/search-kernel/v2/optional-uint32.js";
import { OptionalUInt64T } from "../src/generated/flatbuffers/search-kernel/v2/optional-uint64.js";
import { RelevanceKind } from "../src/generated/flatbuffers/search-kernel/v2/relevance-kind.js";
import { RelevanceT } from "../src/generated/flatbuffers/search-kernel/v2/relevance.js";
import { ResultGroupT } from "../src/generated/flatbuffers/search-kernel/v2/result-group.js";
import { ResultNodeKind } from "../src/generated/flatbuffers/search-kernel/v2/result-node-kind.js";
import { ResultNodeT } from "../src/generated/flatbuffers/search-kernel/v2/result-node.js";
import { SearchResultT } from "../src/generated/flatbuffers/search-kernel/v2/search-result.js";
import { StringEntryT } from "../src/generated/flatbuffers/search-kernel/v2/string-entry.js";
import { TotalHitsRelation } from "../src/generated/flatbuffers/search-kernel/v2/total-hits-relation.js";
import { TotalHitsT } from "../src/generated/flatbuffers/search-kernel/v2/total-hits.js";
import { ValueEntryT } from "../src/generated/flatbuffers/search-kernel/v2/value-entry.js";

const node = (group: ResultGroupT) => new ResultNodeT(ResultNodeKind.ResultGroup, group);

describe("ergonomic search results", () => {
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
      [new ValueEntryT("count", value(3))],
    );
    const facetGroup = new ResultGroupT(
      null,
      "brand",
      null,
      [],
      [node(brandBucket)],
    );
    const groupingContainer = new ResultGroupT(
      null,
      null,
      null,
      [],
      [node(facetGroup)],
    );
    const root = new ResultGroupT(
      null,
      null,
      null,
      [new ValueEntryT("avg_price", value(12.5))],
      [new ResultNodeT(ResultNodeKind.Hit, hit), node(groupingContainer)],
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
    expect(result.facets.brand).toEqual([{ value: "Acme", count: 3n }]);
    expect(result.aggregations.avg_price).toBe(12.5);
    expect(result.totalHits).toBe(10n);
    expect(result.totalHitsRelation).toBe("exact");
    expect(result.coverage?.full).toBe(true);
    expect(result.continuations).toEqual(["token-1"]);
    expect(result.root).not.toBeNull();
  });
});

it("fails closed for mismatched result-node unions", () => {
  const malformed = new ResultGroupT(
    null,
    null,
    null,
    [],
    [new ResultNodeT(ResultNodeKind.Hit, null)],
  );
  expect(() => new SearchResults(new SearchResultT(node(malformed)))).toThrow(/hit node has a missing or mismatched payload/);
});

it("rejects duplicate semantic-map keys in result fields", () => {
  const malformed = new HitT(
    "doc-1",
    null,
    [new ValueEntryT("same", value(1)), new ValueEntryT("same", value(2))],
  );
  const root = new ResultGroupT(null, null, null, [], [new ResultNodeT(ResultNodeKind.Hit, malformed)]);
  expect(() => new SearchResults(new SearchResultT(node(root)))).toThrow(/duplicate result map key/);
});
