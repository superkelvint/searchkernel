import { QueryAndT } from "../generated/flatbuffers/search-kernel/v2/query-and.js";
import { QueryContainsT } from "../generated/flatbuffers/search-kernel/v2/query-contains.js";
import { QueryExprKind } from "../generated/flatbuffers/search-kernel/v2/query-expr-kind.js";
import { QueryExprT } from "../generated/flatbuffers/search-kernel/v2/query-expr.js";
import { QueryFieldRefT } from "../generated/flatbuffers/search-kernel/v2/query-field-ref.js";
import { QueryNearestNeighborT } from "../generated/flatbuffers/search-kernel/v2/query-nearest-neighbor.js";
import { QueryNotT } from "../generated/flatbuffers/search-kernel/v2/query-not.js";
import { QueryOrT } from "../generated/flatbuffers/search-kernel/v2/query-or.js";
import { QuerySourceKind } from "../generated/flatbuffers/search-kernel/v2/query-source-kind.js";
import { QuerySourceT } from "../generated/flatbuffers/search-kernel/v2/query-source.js";
import { TermQueryT } from "../generated/flatbuffers/search-kernel/v2/term-query.js";
import { TypedQueryT } from "../generated/flatbuffers/search-kernel/v2/typed-query.js";
import { value } from "../client/values.js";
export type TypedQuery = QuerySourceT;
function typed(expression: QueryExprT): QuerySourceT { return new QuerySourceT(QuerySourceKind.TypedQuery, new TypedQueryT(expression)); }
function expression(source: TypedQuery): QueryExprT { if (source.sourceType !== QuerySourceKind.TypedQuery || !(source.source instanceof TypedQueryT)) throw new TypeError("typed query clauses must come from q helpers"); return source.source.query!; }
export const q = {
  contains(field: string, text: string): TypedQuery { return typed(new QueryExprT(QueryExprKind.QueryContains, new QueryContainsT(new QueryFieldRefT(field), new QueryExprT(QueryExprKind.TermQuery, new TermQueryT(value(text)))))); },
  allOf(...clauses: TypedQuery[]): TypedQuery { if (!clauses.length) throw new TypeError("q.allOf requires at least one clause"); return typed(new QueryExprT(QueryExprKind.QueryAnd, new QueryAndT(clauses.map(expression)))); },
  anyOf(...clauses: TypedQuery[]): TypedQuery { if (!clauses.length) throw new TypeError("q.anyOf requires at least one clause"); return typed(new QueryExprT(QueryExprKind.QueryOr, new QueryOrT(clauses.map(expression)))); },
  not(clause: TypedQuery): TypedQuery { return typed(new QueryExprT(QueryExprKind.QueryNot, new QueryNotT(expression(clause)))); },
  nearestNeighbor(documentField: string, queryTensor: string): TypedQuery { return typed(new QueryExprT(QueryExprKind.QueryNearestNeighbor, new QueryNearestNeighborT(new QueryFieldRefT(documentField), queryTensor))); },
};
