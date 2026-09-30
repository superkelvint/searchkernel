import { QueryAndT } from "../generated/flatbuffers/search-kernel/v2/query-and.js";
import { QueryContainsT } from "../generated/flatbuffers/search-kernel/v2/query-contains.js";
import { QueryExprKind } from "../generated/flatbuffers/search-kernel/v2/query-expr-kind.js";
import { QueryExprT } from "../generated/flatbuffers/search-kernel/v2/query-expr.js";
import { QueryFieldRefT } from "../generated/flatbuffers/search-kernel/v2/query-field-ref.js";
import { QueryNearestNeighborT } from "../generated/flatbuffers/search-kernel/v2/query-nearest-neighbor.js";
import { QueryNotT } from "../generated/flatbuffers/search-kernel/v2/query-not.js";
import { QueryOrT } from "../generated/flatbuffers/search-kernel/v2/query-or.js";
import { QueryPhraseT } from "../generated/flatbuffers/search-kernel/v2/query-phrase.js";
import { QueryComparisonT } from "../generated/flatbuffers/search-kernel/v2/query-comparison.js";
import { ComparisonOp } from "../generated/flatbuffers/search-kernel/v2/comparison-op.js";
import { QueryRangeT } from "../generated/flatbuffers/search-kernel/v2/query-range.js";
import { QueryInT } from "../generated/flatbuffers/search-kernel/v2/query-in.js";
import { RangeBoundT } from "../generated/flatbuffers/search-kernel/v2/range-bound.js";
import { RangeBoundKind } from "../generated/flatbuffers/search-kernel/v2/range-bound-kind.js";
import { FiniteRangeBoundT } from "../generated/flatbuffers/search-kernel/v2/finite-range-bound.js";
import { OptionalBoolT } from "../generated/flatbuffers/search-kernel/v2/optional-bool.js";
import { QueryFuzzyT } from "../generated/flatbuffers/search-kernel/v2/query-fuzzy.js";
import { QueryWeakAndT } from "../generated/flatbuffers/search-kernel/v2/query-weak-and.js";
import { QueryWandT } from "../generated/flatbuffers/search-kernel/v2/query-wand.js";
import { QueryWeightedSetT } from "../generated/flatbuffers/search-kernel/v2/query-weighted-set.js";
import { QueryDotProductT } from "../generated/flatbuffers/search-kernel/v2/query-dot-product.js";
import { QuerySameElementT } from "../generated/flatbuffers/search-kernel/v2/query-same-element.js";
import { QueryGeoLocationT } from "../generated/flatbuffers/search-kernel/v2/query-geo-location.js";
import { QueryGeoBoundingBoxT } from "../generated/flatbuffers/search-kernel/v2/query-geo-bounding-box.js";
import { WeightedQueryItemT } from "../generated/flatbuffers/search-kernel/v2/weighted-query-item.js";
import { QuerySourceKind } from "../generated/flatbuffers/search-kernel/v2/query-source-kind.js";
import { QuerySourceT } from "../generated/flatbuffers/search-kernel/v2/query-source.js";
import { TermQueryT } from "../generated/flatbuffers/search-kernel/v2/term-query.js";
import { TypedQueryT } from "../generated/flatbuffers/search-kernel/v2/typed-query.js";
import { value } from "../client/values.js";
export type TypedQuery = QuerySourceT;
function typed(expression: QueryExprT): QuerySourceT { return new QuerySourceT(QuerySourceKind.TypedQuery, new TypedQueryT(expression)); }
function expression(source: TypedQuery): QueryExprT { if (source.sourceType !== QuerySourceKind.TypedQuery || !(source.source instanceof TypedQueryT)) throw new TypeError("typed query clauses must come from q helpers"); return source.source.query!; }
function term(item: string): QueryExprT { return new QueryExprT(QueryExprKind.TermQuery, new TermQueryT(value(item))); }
function weightedItems(items: Record<string, number>): WeightedQueryItemT[] {
  return Object.entries(items).sort(([left], [right]) => left.localeCompare(right)).map(([key, weight]) => {
    if (!Number.isSafeInteger(weight)) throw new RangeError("weighted query weights must be safe integers");
    return new WeightedQueryItemT(value(key), BigInt(weight));
  });
}
export const q = {
  contains(field: string, text: string): TypedQuery { return typed(new QueryExprT(QueryExprKind.QueryContains, new QueryContainsT(new QueryFieldRefT(field), term(text)))); },
  phrase(field: string, ...terms: string[]): TypedQuery { if (!terms.length) throw new TypeError("q.phrase requires at least one term"); return typed(new QueryExprT(QueryExprKind.QueryContains, new QueryContainsT(new QueryFieldRefT(field), new QueryExprT(QueryExprKind.QueryPhrase, new QueryPhraseT(terms.map(term)))))); },
  range(field: string, lower: number, upper: number): TypedQuery {
    const bound = (input: number) => new RangeBoundT(RangeBoundKind.FiniteRangeBound, new FiniteRangeBoundT(value(input)));
    return typed(new QueryExprT(QueryExprKind.QueryRange, new QueryRangeT(new QueryFieldRefT(field), bound(lower), bound(upper), new OptionalBoolT(true), new OptionalBoolT(true))));
  },
  inValues(field: string, values: Array<string | number | boolean>): TypedQuery { if (!values.length) throw new TypeError("q.inValues requires at least one value"); return typed(new QueryExprT(QueryExprKind.QueryIn, new QueryInT(new QueryFieldRefT(field), values.map(value)))); },
  equals(field: string, item: string | number | boolean): TypedQuery { return typed(new QueryExprT(QueryExprKind.QueryComparison, new QueryComparisonT(new QueryFieldRefT(field), ComparisonOp.Eq, value(item)))); },
  fuzzy(field: string, item: string): TypedQuery { return typed(new QueryExprT(QueryExprKind.QueryContains, new QueryContainsT(new QueryFieldRefT(field), new QueryExprT(QueryExprKind.QueryFuzzy, new QueryFuzzyT(value(item)))))); },
  weakAnd(...clauses: TypedQuery[]): TypedQuery { if (!clauses.length) throw new TypeError("q.weakAnd requires at least one clause"); return typed(new QueryExprT(QueryExprKind.QueryWeakAnd, new QueryWeakAndT(clauses.map(expression)))); },
  wand(field: string, items: Record<string, number>): TypedQuery { if (!Object.keys(items).length) throw new TypeError("q.wand requires at least one weighted item"); return typed(new QueryExprT(QueryExprKind.QueryWand, new QueryWandT(new QueryFieldRefT(field), weightedItems(items)))); },
  weightedSet(field: string, items: Record<string, number>): TypedQuery { if (!Object.keys(items).length) throw new TypeError("q.weightedSet requires at least one weighted item"); return typed(new QueryExprT(QueryExprKind.QueryWeightedSet, new QueryWeightedSetT(new QueryFieldRefT(field), weightedItems(items)))); },
  dotProduct(field: string, items: Record<string, number>): TypedQuery { if (!Object.keys(items).length) throw new TypeError("q.dotProduct requires at least one weighted item"); return typed(new QueryExprT(QueryExprKind.QueryDotProduct, new QueryDotProductT(new QueryFieldRefT(field), weightedItems(items)))); },
  sameElement(field: string, ...clauses: TypedQuery[]): TypedQuery { if (!clauses.length) throw new TypeError("q.sameElement requires at least one clause"); return typed(new QueryExprT(QueryExprKind.QueryContains, new QueryContainsT(new QueryFieldRefT(field), new QueryExprT(QueryExprKind.QuerySameElement, new QuerySameElementT(clauses.map(expression)))))); },
  geoLocation(field: string, latitude: number, longitude: number, radius: string): TypedQuery { return typed(new QueryExprT(QueryExprKind.QueryGeoLocation, new QueryGeoLocationT(new QueryFieldRefT(field), latitude, longitude, radius))); },
  geoBox(field: string, south: number, west: number, north: number, east: number): TypedQuery { return typed(new QueryExprT(QueryExprKind.QueryGeoBoundingBox, new QueryGeoBoundingBoxT(new QueryFieldRefT(field), south, west, north, east))); },
  allOf(...clauses: TypedQuery[]): TypedQuery { if (!clauses.length) throw new TypeError("q.allOf requires at least one clause"); return typed(new QueryExprT(QueryExprKind.QueryAnd, new QueryAndT(clauses.map(expression)))); },
  anyOf(...clauses: TypedQuery[]): TypedQuery { if (!clauses.length) throw new TypeError("q.anyOf requires at least one clause"); return typed(new QueryExprT(QueryExprKind.QueryOr, new QueryOrT(clauses.map(expression)))); },
  not(clause: TypedQuery): TypedQuery { return typed(new QueryExprT(QueryExprKind.QueryNot, new QueryNotT(expression(clause)))); },
  nearestNeighbor(documentField: string, queryTensor: string): TypedQuery { return typed(new QueryExprT(QueryExprKind.QueryNearestNeighbor, new QueryNearestNeighborT(new QueryFieldRefT(documentField), queryTensor))); },
};
