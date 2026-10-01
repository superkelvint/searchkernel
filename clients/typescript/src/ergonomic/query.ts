import { QueryAndT } from "../semantic.js";
import { QueryContainsT } from "../semantic.js";
import { QueryExprKind } from "../semantic.js";
import { QueryExprT } from "../semantic.js";
import { QueryFieldRefT } from "../semantic.js";
import { QueryNearestNeighborT } from "../semantic.js";
import { QueryNotT } from "../semantic.js";
import { QueryOrT } from "../semantic.js";
import { QueryPhraseT } from "../semantic.js";
import { QueryNearT } from "../semantic.js";
import { QueryONearT } from "../semantic.js";
import { QueryComparisonT } from "../semantic.js";
import { ComparisonOp } from "../semantic.js";
import { QueryRangeT } from "../semantic.js";
import { QueryInT } from "../semantic.js";
import { RangeBoundT } from "../semantic.js";
import { RangeBoundKind } from "../semantic.js";
import { FiniteRangeBoundT } from "../semantic.js";
import { OptionalBoolT } from "../semantic.js";
import { QueryFuzzyT } from "../semantic.js";
import { QueryWeakAndT } from "../semantic.js";
import { QueryWandT } from "../semantic.js";
import { QueryWeightedSetT } from "../semantic.js";
import { QueryDotProductT } from "../semantic.js";
import { QuerySameElementT } from "../semantic.js";
import { QueryGeoLocationT } from "../semantic.js";
import { QueryMatchesT } from "../semantic.js";
import { QueryGeoBoundingBoxT } from "../semantic.js";
import { WeightedQueryItemT } from "../semantic.js";
import { QuerySourceKind } from "../semantic.js";
import { QuerySourceT } from "../semantic.js";
import { TermQueryT } from "../semantic.js";
import { TypedQueryT } from "../semantic.js";
import type { ValueT } from "../semantic.js";
import { value } from "../client/values.js";
export type TypedQuery = QuerySourceT;
function typed(expression: QueryExprT): QuerySourceT { return new QuerySourceT(QuerySourceKind.TypedQuery, new TypedQueryT(expression)); }
function expression(source: TypedQuery): QueryExprT { if (source.sourceType !== QuerySourceKind.TypedQuery || !(source.source instanceof TypedQueryT)) throw new TypeError("typed query clauses must come from q helpers"); return source.source.query!; }
function termExpr(item: string): QueryExprT { return new QueryExprT(QueryExprKind.TermQuery, new TermQueryT(value(item))); }
function literalRegex(item: string): string {
  const specials = new Set("\\.^$|?*+()[]{}".split(""));
  return [...item].map((character) => specials.has(character) ? `\\${character}` : character).join("");
}
function weightedItems(items: Record<string, number>): WeightedQueryItemT[] {
  return Object.entries(items).sort(([left], [right]) => left.localeCompare(right)).map(([key, weight]) => {
    if (!Number.isSafeInteger(weight)) throw new RangeError("weighted query weights must be safe integers");
    return new WeightedQueryItemT(value(key), BigInt(weight));
  });
}
export const q = {
  term(field: string, text: string): TypedQuery { return typed(new QueryExprT(QueryExprKind.QueryContains, new QueryContainsT(new QueryFieldRefT(field), termExpr(text)))); },
  contains(field: string, text: string): TypedQuery { return typed(new QueryExprT(QueryExprKind.QueryContains, new QueryContainsT(new QueryFieldRefT(field), termExpr(text)))); },
  phrase(field: string, ...terms: string[]): TypedQuery { if (!terms.length) throw new TypeError("q.phrase requires at least one term"); return typed(new QueryExprT(QueryExprKind.QueryContains, new QueryContainsT(new QueryFieldRefT(field), new QueryExprT(QueryExprKind.QueryPhrase, new QueryPhraseT(terms.map(termExpr)))))); },
  near(field: string, ...terms: string[]): TypedQuery { if (!terms.length) throw new TypeError("q.near requires at least one term"); return typed(new QueryExprT(QueryExprKind.QueryContains, new QueryContainsT(new QueryFieldRefT(field), new QueryExprT(QueryExprKind.QueryNear, new QueryNearT(terms.map(termExpr)))))); },
  orderedNear(field: string, ...terms: string[]): TypedQuery { if (!terms.length) throw new TypeError("q.orderedNear requires at least one term"); return typed(new QueryExprT(QueryExprKind.QueryContains, new QueryContainsT(new QueryFieldRefT(field), new QueryExprT(QueryExprKind.QueryONear, new QueryONearT(terms.map(termExpr)))))); },
  range(field: string, lower: number | ValueT, upper: number | ValueT, options: { includeLower?: boolean; includeUpper?: boolean } = {}): TypedQuery {
    const bound = (input: number | ValueT) => new RangeBoundT(RangeBoundKind.FiniteRangeBound, new FiniteRangeBoundT(value(input)));
    return typed(new QueryExprT(QueryExprKind.QueryRange, new QueryRangeT(
      new QueryFieldRefT(field), bound(lower), bound(upper),
      new OptionalBoolT(options.includeLower ?? true), new OptionalBoolT(options.includeUpper ?? true),
    )));
  },
  matches(field: string, pattern: string): TypedQuery {
    return typed(new QueryExprT(QueryExprKind.QueryMatches, new QueryMatchesT(new QueryFieldRefT(field), pattern)));
  },
  prefix(field: string, text: string): TypedQuery { return q.matches(field, `${literalRegex(text)}.*`); },
  substring(field: string, text: string): TypedQuery { return q.matches(field, `.*${literalRegex(text)}.*`); },
  suffix(field: string, text: string): TypedQuery { return q.matches(field, `.*${literalRegex(text)}`); },
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
