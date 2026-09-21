"""Idiomatic Python builders lowered directly to generated V2 objects."""

from __future__ import annotations

from dataclasses import dataclass
from typing import Any, Mapping

from SearchKernel.V2.AggregationKind import AggregationKind
from SearchKernel.V2.AvgAggregation import AvgAggregationT
from SearchKernel.V2.CountAggregation import CountAggregationT
from SearchKernel.V2.FieldDefinition import FieldDefinitionT
from SearchKernel.V2.DataType import DataTypeT
from SearchKernel.V2.DataTypeKind import DataTypeKind
from SearchKernel.V2.TensorDataType import TensorDataTypeT
from SearchKernel.V2.RankPhaseDefinition import RankPhaseDefinitionT
from SearchKernel.V2.RankProfileDefinition import RankProfileDefinitionT
from SearchKernel.V2.RankProfileInput import RankProfileInputT
from SearchKernel.V2.GroupingBinaryOperator import GroupingBinaryOperator
from SearchKernel.V2.FilterClause import FilterClauseT
from SearchKernel.V2.FilterRequest import FilterRequestT
from SearchKernel.V2.GroupingAggregationExpression import GroupingAggregationExpressionT
from SearchKernel.V2.GroupingAliasOperation import GroupingAliasOperationT
from SearchKernel.V2.GroupingAliasReference import GroupingAliasReferenceT
from SearchKernel.V2.GroupingAllOperation import GroupingAllOperationT
from SearchKernel.V2.GroupingAttributeExpression import GroupingAttributeExpressionT
from SearchKernel.V2.GroupingBinaryExpression import GroupingBinaryExpressionT
from SearchKernel.V2.GroupingEachOperation import GroupingEachOperationT
from SearchKernel.V2.GroupingExpression import GroupingExpressionT
from SearchKernel.V2.GroupingExpressionKind import GroupingExpressionKind
from SearchKernel.V2.GroupingFilterAnd import GroupingFilterAndT
from SearchKernel.V2.GroupingFilterExpr import GroupingFilterExprT
from SearchKernel.V2.GroupingFilterExprKind import GroupingFilterExprKind
from SearchKernel.V2.GroupingFilterIsTrue import GroupingFilterIsTrueT
from SearchKernel.V2.GroupingFilterNot import GroupingFilterNotT
from SearchKernel.V2.GroupingFilterOr import GroupingFilterOrT
from SearchKernel.V2.GroupingFilterRegex import GroupingFilterRegexT
from SearchKernel.V2.GroupingFunctionExpression import GroupingFunctionExpressionT
from SearchKernel.V2.GroupingGroupOperation import GroupingGroupOperationT
from SearchKernel.V2.GroupingKeepOperation import GroupingKeepOperationT
from SearchKernel.V2.GroupingLiteralExpression import GroupingLiteralExpressionT
from SearchKernel.V2.GroupingMaxKind import GroupingMaxKind
from SearchKernel.V2.GroupingMaxOperation import GroupingMaxOperationT
from SearchKernel.V2.GroupingOperation import GroupingOperationT
from SearchKernel.V2.GroupingOperationKind import GroupingOperationKind
from SearchKernel.V2.GroupingOrderExpr import GroupingOrderExprT
from SearchKernel.V2.GroupingOrderOperation import GroupingOrderOperationT
from SearchKernel.V2.GroupingOutput import GroupingOutputT
from SearchKernel.V2.GroupingOutputOperation import GroupingOutputOperationT
from SearchKernel.V2.GroupingPrecisionOperation import GroupingPrecisionOperationT
from SearchKernel.V2.GroupingRequest import GroupingRequestT
from SearchKernel.V2.GroupingSourceKind import GroupingSourceKind
from SearchKernel.V2.GroupingSourceKind import GroupingSourceKind as GroupingSourceTags
from SearchKernel.V2.GroupingUnaryExpression import GroupingUnaryExpressionT
from SearchKernel.V2.GroupingUnaryOperator import GroupingUnaryOperator
from SearchKernel.V2.FiniteGroupingMax import FiniteGroupingMaxT
from SearchKernel.V2.StructuredGroupingSource import StructuredGroupingSourceT
from SearchKernel.V2.PresentationRequest import PresentationRequestT
from SearchKernel.V2.FieldProjection import FieldProjectionT
from SearchKernel.V2.HighlightRequest import HighlightRequestT
from SearchKernel.V2.QueryAnd import QueryAndT
from SearchKernel.V2.QueryContains import QueryContainsT
from SearchKernel.V2.QueryExpr import QueryExprT
from SearchKernel.V2.QueryExprKind import QueryExprKind
from SearchKernel.V2.QueryFuzzy import QueryFuzzyT
from SearchKernel.V2.QueryGeoBoundingBox import QueryGeoBoundingBoxT
from SearchKernel.V2.QueryGeoLocation import QueryGeoLocationT
from SearchKernel.V2.QueryFieldRef import QueryFieldRefT
from SearchKernel.V2.QueryNear import QueryNearT
from SearchKernel.V2.QueryNearestNeighbor import QueryNearestNeighborT
from SearchKernel.V2.QueryNot import QueryNotT
from SearchKernel.V2.QueryONear import QueryONearT
from SearchKernel.V2.QueryOr import QueryOrT
from SearchKernel.V2.QueryPhrase import QueryPhraseT
from SearchKernel.V2.QueryRank import QueryRankT
from SearchKernel.V2.QuerySameElement import QuerySameElementT
from SearchKernel.V2.QueryInput import QueryInputT
from SearchKernel.V2.QueryInputKind import QueryInputKind
from SearchKernel.V2.LiteralQueryInput import LiteralQueryInputT
from SearchKernel.V2.ExpressionQueryInput import ExpressionQueryInputT
from SearchKernel.V2.MatchingMode import MatchingMode
from SearchKernel.V2.PrimitiveDataTypeKind import PrimitiveDataTypeKind
from SearchKernel.V2.QuerySource import QuerySourceT
from SearchKernel.V2.QuerySourceKind import QuerySourceKind
from SearchKernel.V2.SearchField import SearchFieldT
from SearchKernel.V2.SearchFieldSelection import SearchFieldSelectionT
from SearchKernel.V2.SearchKernelTextQuery import SearchKernelTextQueryT
from SearchKernel.V2.MatchAllQuery import MatchAllQueryT
from SearchKernel.V2.YqlQuery import YqlQueryT
from SearchKernel.V2.TypedQuery import TypedQueryT
from SearchKernel.V2.QueryWand import QueryWandT
from SearchKernel.V2.QueryWeakAnd import QueryWeakAndT
from SearchKernel.V2.WeightedQueryItem import WeightedQueryItemT
from SearchKernel.V2.RankingRequest import RankingRequestT
from SearchKernel.V2.SearchRequest import SearchRequestT
from SearchKernel.V2.OptionalUInt32 import OptionalUInt32T
from SearchKernel.V2.SortDirection import SortDirection
from SearchKernel.V2.SortExpr import SortExprT
from SearchKernel.V2.SortTarget import SortTargetT
from SearchKernel.V2.SortTargetKind import SortTargetKind
from SearchKernel.V2.FieldSortTarget import FieldSortTargetT
from SearchKernel.V2.StringEntry import StringEntryT
from SearchKernel.V2.ValueEntry import ValueEntryT
from SearchKernel.V2.Value import ValueT
from SearchKernel.V2.SetField import SetFieldT
from SearchKernel.V2.UnsetField import UnsetFieldT
from SearchKernel.V2.PatchOperation import PatchOperationT
from SearchKernel.V2.PatchOperationKind import PatchOperationKind
from SearchKernel.V2.PatchRequest import PatchRequestT
from SearchKernel.V2.ArithmeticUpdate import ArithmeticUpdateT
from SearchKernel.V2.ArithmeticUpdateOperator import ArithmeticUpdateOperator
from SearchKernel.V2.AssignUpdate import AssignUpdateT
from SearchKernel.V2.ClearUpdate import ClearUpdateT
from SearchKernel.V2.FieldPath import FieldPathT
from SearchKernel.V2.OptionalBool import OptionalBoolT
from SearchKernel.V2.OptionalUInt32 import OptionalUInt32T
from SearchKernel.V2.AttributeConfig import AttributeConfigT
from SearchKernel.V2.DistanceMetric import DistanceMetric
from SearchKernel.V2.HnswIndexConfig import HnswIndexConfigT
from SearchKernel.V2.IndexConfig import IndexConfigT
from SearchKernel.V2.UpdateOperation import UpdateOperationT
from SearchKernel.V2.UpdateOperationKind import UpdateOperationKind
from SearchKernel.V2.UpdateRequest import UpdateRequestT
from SearchKernel.V2.ResultNodeKind import ResultNodeKind

from .client import Tensor, _python_value, value


class _FieldSpec:
    kind = "string"
    matching = "unspecified"

    def __init__(self, *, stored=None, searchable=None, filterable=None, sortable=None, facetable=None):
        self.stored, self.searchable = stored, searchable
        self.filterable, self.sortable, self.facetable = filterable, sortable, facetable

    def definition(self, name: str) -> FieldDefinitionT:
        primitive = {
            "string": PrimitiveDataTypeKind.String,
            "text": PrimitiveDataTypeKind.String,
            "keyword": PrimitiveDataTypeKind.String,
            "int64": PrimitiveDataTypeKind.Int64,
            "float64": PrimitiveDataTypeKind.Float64,
            "bool": PrimitiveDataTypeKind.Bool,
        }[self.kind]
        optional = lambda setting: None if setting is None else __import__(
            "SearchKernel.V2.OptionalBool", fromlist=["OptionalBoolT"]
        ).OptionalBoolT(value=setting)
        return FieldDefinitionT(
            name=name,
            dataType=DataTypeT(kindType=DataTypeKind.PrimitiveDataType, kind=__import__(
                "SearchKernel.V2.PrimitiveDataType", fromlist=["PrimitiveDataTypeT"]
            ).PrimitiveDataTypeT(kind=primitive)),
            stored=optional(self.stored), searchable=optional(self.searchable),
            filterable=optional(self.filterable), sortable=optional(self.sortable),
            facetable=optional(self.facetable),
            matching=(
                __import__("SearchKernel.V2.MatchingConfig", fromlist=["MatchingConfigT"]).MatchingConfigT(
                    mode={
                        "unspecified": MatchingMode.Unspecified,
                        "text": MatchingMode.Text,
                        "exact": MatchingMode.Exact,
                    }.get(self.matching, MatchingMode.Unspecified)
                )
                if primitive == PrimitiveDataTypeKind.String
                else None
            ),
        )


class Text(_FieldSpec):
    kind, matching = "text", "text"


class Keyword(_FieldSpec):
    kind, matching = "keyword", "exact"


class Integer(_FieldSpec):
    kind = "int64"


class Float(_FieldSpec):
    kind = "float64"


class Boolean(_FieldSpec):
    kind = "bool"


@dataclass(frozen=True)
class HnswIndexConfig:
    """Portable HNSW construction settings for a tensor field."""

    max_links_per_node: int | None = None
    neighbors_to_explore_at_insert: int | None = None

    def _generated(self) -> HnswIndexConfigT:
        optional = lambda setting: None if setting is None else OptionalUInt32T(value=setting)
        return HnswIndexConfigT(
            maxLinksPerNode=optional(self.max_links_per_node),
            neighborsToExploreAtInsert=optional(self.neighbors_to_explore_at_insert),
        )


class TensorField(_FieldSpec):
    kind = "tensor"

    def __init__(self, type_spec: str, *, distance_metric=None, hnsw=None, **kwargs):
        super().__init__(**kwargs)
        self.type_spec = type_spec
        self.distance_metric = distance_metric
        self.hnsw = hnsw

    def definition(self, name: str) -> FieldDefinitionT:
        if not isinstance(self.type_spec, str) or not self.type_spec.strip():
            raise ValueError("tensor type spec must be a non-empty string")
        optional = lambda setting: None if setting is None else __import__(
            "SearchKernel.V2.OptionalBool", fromlist=["OptionalBoolT"]
        ).OptionalBoolT(value=setting)
        metric = None
        if self.distance_metric is not None:
            metric = {
                "euclidean": DistanceMetric.Euclidean,
                "angular": DistanceMetric.Angular,
                "dot_product": DistanceMetric.DotProduct,
                "dotproduct": DistanceMetric.DotProduct,
                "prenormalized_angular": DistanceMetric.PrenormalizedAngular,
                "geo_degrees": DistanceMetric.GeoDegrees,
                "geodegrees": DistanceMetric.GeoDegrees,
                "hamming": DistanceMetric.Hamming,
            }.get(str(self.distance_metric).lower())
            if metric is None:
                raise ValueError(f"unsupported tensor distance metric: {self.distance_metric}")
        hnsw = self.hnsw._generated() if self.hnsw is not None else None
        return FieldDefinitionT(
            name=name,
            dataType=DataTypeT(
                kindType=DataTypeKind.TensorDataType,
                kind=TensorDataTypeT(typeSpec=self.type_spec),
            ),
            stored=optional(self.stored),
            searchable=optional(self.searchable),
            filterable=optional(self.filterable),
            sortable=optional(self.sortable),
            facetable=optional(self.facetable),
            index=IndexConfigT(
                enabled=OptionalBoolT(value=True),
                hnsw=hnsw,
            ) if hnsw is not None else None,
            attribute=AttributeConfigT(
                enabled=OptionalBoolT(value=True),
                distanceMetric=metric or DistanceMetric.Unspecified,
            ) if metric is not None or hnsw is not None else None,
        )


class RankProfile:
    """Small ergonomic builder for the rank-profile fields used by the client."""

    def __init__(self, name: str):
        if not name:
            raise ValueError("rank profile name must not be empty")
        self.name = name
        self._first_phase = None
        self._second_phase = None
        self._rerank_count = None
        self._match_features = []
        self._query_inputs = {}

    def first_phase(self, expression: str) -> "RankProfile":
        self._first_phase = expression
        return self

    def second_phase(self, expression: str) -> "RankProfile":
        self._second_phase = expression
        return self

    def rerank_count(self, count: int) -> "RankProfile":
        if isinstance(count, bool) or not isinstance(count, int) or not 0 <= count <= 0xFFFF_FFFF:
            raise ValueError("rerank count must be a uint32")
        self._rerank_count = count
        return self

    def match_feature(self, expression: str) -> "RankProfile":
        self._match_features.append(expression)
        return self

    def query_input(self, name: str, type_spec: str) -> "RankProfile":
        self._query_inputs[name] = type_spec
        return self

    def definition(self) -> RankProfileDefinitionT:
        inputs = [
            RankProfileInputT(
                name=name,
                dataType=DataTypeT(
                    kindType=DataTypeKind.TensorDataType,
                    kind=TensorDataTypeT(typeSpec=type_spec),
                ),
                defaultValue=value(Tensor.dense(type_spec, [])),
            )
            for name, type_spec in sorted(self._query_inputs.items())
        ]
        first_phase = (
            RankPhaseDefinitionT(expression=self._first_phase)
            if self._first_phase is not None
            else None
        )
        second_phase = (
            RankPhaseDefinitionT(
                expression=self._second_phase,
                rerankCount=(
                    OptionalUInt32T(value=self._rerank_count)
                    if self._rerank_count is not None
                    else None
                ),
            )
            if self._second_phase is not None or self._rerank_count is not None
            else None
        )
        return RankProfileDefinitionT(
            name=self.name,
            inputs=inputs or None,
            firstPhase=first_phase,
            secondPhase=second_phase,
            matchFeatures=list(self._match_features) or None,
        )


def _expr_union(tag: int, payload: Any) -> GroupingExpressionT:
    return GroupingExpressionT(exprType=tag, expr=payload)


@dataclass(frozen=True)
class _Aggregation:
    kind: str
    field: str | None = None
    summary: str | None = None


@dataclass(frozen=True)
class _GroupingExpr:
    model: GroupingExpressionT

    def as_(self, name: str) -> "_Output":
        return _Output(self, name)

    def __neg__(self):
        return _GroupingExpr(_expr_union(GroupingExpressionKind.GroupingUnaryExpression, GroupingUnaryExpressionT(op=GroupingUnaryOperator.Negative, expression=self.model)))

    def __add__(self, other): return _binary(GroupingBinaryOperator.Add, self, other)
    def __sub__(self, other): return _binary(GroupingBinaryOperator.Subtract, self, other)
    def __mul__(self, other): return _binary(GroupingBinaryOperator.Multiply, self, other)
    def __truediv__(self, other): return _binary(GroupingBinaryOperator.Divide, self, other)


@dataclass(frozen=True)
class _Output:
    expression: _GroupingExpr
    label: str


@dataclass(frozen=True)
class _Order:
    expression: _GroupingExpr


def _as_expr(item: Any) -> _GroupingExpr:
    return item if isinstance(item, _GroupingExpr) else g.value(item)


def _binary(op: int, left: Any, right: Any) -> _GroupingExpr:
    return _GroupingExpr(_expr_union(GroupingExpressionKind.GroupingBinaryExpression, GroupingBinaryExpressionT(op=op, left=_as_expr(left).model, right=_as_expr(right).model)))


class _GroupingNamespace:
    def field(self, name: str) -> _GroupingExpr:
        return _GroupingExpr(_expr_union(GroupingExpressionKind.GroupingAttributeExpression, GroupingAttributeExpressionT(field=name)))

    def value(self, item: Any) -> _GroupingExpr:
        return _GroupingExpr(_expr_union(GroupingExpressionKind.GroupingLiteralExpression, GroupingLiteralExpressionT(value=value(item))))

    def fn(self, name: str, *arguments: Any) -> _GroupingExpr:
        return _GroupingExpr(_expr_union(GroupingExpressionKind.GroupingFunctionExpression, GroupingFunctionExpressionT(name=name, arguments=[_as_expr(a).model for a in arguments])))

    def alias_ref(self, name: str) -> _GroupingExpr:
        return _GroupingExpr(_expr_union(GroupingExpressionKind.GroupingAliasReference, GroupingAliasReferenceT(name=name)))

    def member(self, base: Any, name: str) -> _GroupingExpr:
        from SearchKernel.V2.GroupingMemberExpression import GroupingMemberExpressionT
        return _GroupingExpr(_expr_union(GroupingExpressionKind.GroupingMemberExpression, GroupingMemberExpressionT(base=_as_expr(base).model, member=name)))

    def lookup(self, base: Any, key: Any) -> _GroupingExpr:
        from SearchKernel.V2.GroupingMapLookupExpression import GroupingMapLookupExpressionT
        return _GroupingExpr(_expr_union(GroupingExpressionKind.GroupingMapLookupExpression, GroupingMapLookupExpressionT(base=_as_expr(base).model, key=_as_expr(key).model)))


g = _GroupingNamespace()


class _AggregationNamespace:
    def _make(self, kind: str, field: str | None = None, summary: str | None = None) -> _GroupingExpr:
        payloads = {
            "count": (AggregationKind.CountAggregation, CountAggregationT()),
            "sum": (AggregationKind.SumAggregation, __import__("SearchKernel.V2.SumAggregation", fromlist=["SumAggregationT"]).SumAggregationT(expression=g.field(field).model)),
            "avg": (AggregationKind.AvgAggregation, AvgAggregationT(expression=g.field(field).model)),
            "min": (AggregationKind.MinAggregation, __import__("SearchKernel.V2.MinAggregation", fromlist=["MinAggregationT"]).MinAggregationT(expression=g.field(field).model)),
            "max": (AggregationKind.MaxAggregation, __import__("SearchKernel.V2.MaxAggregation", fromlist=["MaxAggregationT"]).MaxAggregationT(expression=g.field(field).model)),
            "stddev": (AggregationKind.StdDevAggregation, __import__("SearchKernel.V2.StdDevAggregation", fromlist=["StdDevAggregationT"]).StdDevAggregationT(expression=g.field(field).model)),
            "summary": (AggregationKind.SummaryAggregation, __import__("SearchKernel.V2.SummaryAggregation", fromlist=["SummaryAggregationT"]).SummaryAggregationT(summary=summary)),
        }
        tag, payload = payloads[kind]
        return _GroupingExpr(_expr_union(GroupingExpressionKind.GroupingAggregationExpression, GroupingAggregationExpressionT(aggregationType=tag, aggregation=payload)))

    def count(self): return self._make("count")
    def sum(self, field): return self._make("sum", field)
    def avg(self, field): return self._make("avg", field)
    def min(self, field): return self._make("min", field)
    def max(self, field): return self._make("max", field)
    def stddev(self, field): return self._make("stddev", field)
    def summary(self, name=None): return self._make("summary", summary=name)


agg = _AggregationNamespace()


def asc(expression): return _Order(_as_expr(expression))
def desc(expression): return _Order(-_as_expr(expression))


class _GroupingFilterNamespace:
    def regex(self, expression, pattern):
        return GroupingFilterExprT(exprType=GroupingFilterExprKind.GroupingFilterRegex, expr=GroupingFilterRegexT(expression=_as_expr(expression).model, pattern=pattern))

    def is_true(self, expression):
        return GroupingFilterExprT(exprType=GroupingFilterExprKind.GroupingFilterIsTrue, expr=GroupingFilterIsTrueT(expression=_as_expr(expression).model))

    def all_of(self, *clauses): return GroupingFilterExprT(exprType=GroupingFilterExprKind.GroupingFilterAnd, expr=GroupingFilterAndT(clauses=list(clauses)))
    def any_of(self, *clauses): return GroupingFilterExprT(exprType=GroupingFilterExprKind.GroupingFilterOr, expr=GroupingFilterOrT(clauses=list(clauses)))
    def negate(self, clause): return GroupingFilterExprT(exprType=GroupingFilterExprKind.GroupingFilterNot, expr=GroupingFilterNotT(clause=clause))


gf = _GroupingFilterNamespace()


class _QueryNamespace:
    @staticmethod
    def _typed(expression: QueryExprT) -> QuerySourceT:
        return QuerySourceT(
            sourceType=QuerySourceKind.TypedQuery,
            source=TypedQueryT(query=expression),
        )

    @staticmethod
    def _expression(source: QuerySourceT) -> QueryExprT:
        if isinstance(source, QueryExprT):
            return source
        if source.sourceType != QuerySourceKind.TypedQuery or source.source is None:
            raise TypeError("typed query clauses must come from q helpers")
        return source.source.query

    @staticmethod
    def _term(item: Any) -> QueryExprT:
        if isinstance(item, str):
            item = value(item)
        return QueryExprT(
            exprType=QueryExprKind.TermQuery,
            expr=__import__("SearchKernel.V2.TermQuery", fromlist=["TermQueryT"]).TermQueryT(
                value=item if isinstance(item, ValueT) else value(item)
            ),
        )

    def term(self, item: Any) -> QueryExprT:
        return self._term(item)

    def contains(self, field: str, item: Any) -> QuerySourceT:
        if isinstance(item, str):
            item = self._term(item)
        contains = QueryExprT(
            exprType=QueryExprKind.QueryContains,
            expr=QueryContainsT(
                field=QueryFieldRefT(path=field),
                item=self._expression(item),
            ),
        )
        return self._typed(contains)

    def _children(self, items: tuple[Any, ...]) -> list[QueryExprT]:
        if not items:
            raise ValueError("query operator requires at least one item")
        return [self._expression(item if not isinstance(item, str) else self._term(item)) for item in items]

    def phrase(self, *items: Any) -> QueryExprT:
        return QueryExprT(
            exprType=QueryExprKind.QueryPhrase,
            expr=QueryPhraseT(terms=self._children(items)),
        )

    def near(self, *items: Any) -> QueryExprT:
        return QueryExprT(
            exprType=QueryExprKind.QueryNear,
            expr=QueryNearT(terms=self._children(items)),
        )

    def onear(self, *items: Any) -> QueryExprT:
        return QueryExprT(
            exprType=QueryExprKind.QueryONear,
            expr=QueryONearT(terms=self._children(items)),
        )

    def same_element(self, *clauses: Any) -> QuerySourceT:
        # The containing q.contains("array_field", ...) supplies the parent
        # field context used to qualify relative struct-member clauses.
        return self._combine(
            QueryExprKind.QuerySameElement,
            QuerySameElementT(clauses=self._children(clauses)),
        )

    def fuzzy(self, item: Any) -> QueryExprT:
        return QueryExprT(
            exprType=QueryExprKind.QueryFuzzy,
            expr=QueryFuzzyT(value=item if isinstance(item, ValueT) else value(item)),
        )

    def _combine(self, expression_type: int, expression: QueryExprT) -> QuerySourceT:
        return self._typed(QueryExprT(exprType=expression_type, expr=expression))

    def all_of(self, *clauses: QuerySourceT) -> QuerySourceT:
        if not clauses:
            raise ValueError("q.all_of requires at least one clause")
        return self._combine(
            QueryExprKind.QueryAnd,
            QueryAndT(clauses=[self._expression(clause) for clause in clauses]),
        )

    def any_of(self, *clauses: QuerySourceT) -> QuerySourceT:
        if not clauses:
            raise ValueError("q.any_of requires at least one clause")
        return self._combine(
            QueryExprKind.QueryOr,
            QueryOrT(clauses=[self._expression(clause) for clause in clauses]),
        )

    def negate(self, clause: QuerySourceT) -> QuerySourceT:
        return self._combine(
            QueryExprKind.QueryNot,
            QueryNotT(clause=self._expression(clause)),
        )

    def weak_and(self, *clauses: QuerySourceT) -> QuerySourceT:
        return self._combine(
            QueryExprKind.QueryWeakAnd,
            QueryWeakAndT(clauses=self._children(clauses)),
        )

    def wand(self, field: str, items: Mapping[Any, int]) -> QuerySourceT:
        if not items:
            raise ValueError("q.wand requires at least one weighted item")
        weighted = [
            WeightedQueryItemT(key=value(key), weight=weight)
            for key, weight in sorted(items.items(), key=lambda item: str(item[0]))
        ]
        return self._combine(
            QueryExprKind.QueryWand,
            QueryWandT(field=QueryFieldRefT(path=field), items=weighted),
        )

    def rank(self, match: QuerySourceT, *ranking: QuerySourceT) -> QuerySourceT:
        return self._combine(
            QueryExprKind.QueryRank,
            QueryRankT(
                match=self._expression(match),
                rank=self._children(ranking),
            ),
        )

    def geo_location(
        self, field: str, *, lat: float, lon: float, radius: str
    ) -> QuerySourceT:
        return self._combine(
            QueryExprKind.QueryGeoLocation,
            QueryGeoLocationT(
                field=QueryFieldRefT(path=field),
                latitude=lat,
                longitude=lon,
                radius=radius,
            ),
        )

    def geo_box(
        self,
        field: str,
        *,
        south: float,
        west: float,
        north: float,
        east: float,
    ) -> QuerySourceT:
        return self._combine(
            QueryExprKind.QueryGeoBoundingBox,
            QueryGeoBoundingBoxT(
                field=QueryFieldRefT(path=field),
                south=south,
                west=west,
                north=north,
                east=east,
            ),
        )

    def nearest_neighbor(
        self,
        document_field: str,
        query_tensor: str,
        *,
        approximate: bool | None = None,
        target_hits: int | None = None,
        total_target_hits: int | None = None,
        explore_additional_hits: int | None = None,
        distance_threshold: float | None = None,
    ) -> QuerySourceT:
        annotations = {}
        if approximate is not None:
            annotations["approximate"] = approximate
        if target_hits is not None:
            annotations["targetNumHits"] = target_hits
        if total_target_hits is not None:
            annotations["totalTargetHits"] = total_target_hits
        if explore_additional_hits is not None:
            annotations["exploreAdditionalHits"] = explore_additional_hits
        if distance_threshold is not None:
            annotations["distanceThreshold"] = distance_threshold
        nearest = QueryExprT(
            exprType=QueryExprKind.QueryNearestNeighbor,
            expr=QueryNearestNeighborT(
                documentField=QueryFieldRefT(path=document_field),
                queryTensor=query_tensor,
            ),
            annotations=[
                ValueEntryT(key=key, value=value(item))
                for key, item in sorted(annotations.items())
            ] or None,
        )
        return self._typed(nearest)


q = _QueryNamespace()


class Group:
    def __init__(self, field: str | _GroupingExpr):
        self.expression = g.field(field) if isinstance(field, str) else field
        self._limit = None
        self._precision = None
        self._orders, self._outputs, self._subgroups, self._aliases = [], [], [], []
        self._keep = self._filter = None

    @classmethod
    def by(cls, field): return cls(field)
    def limit(self, value): self._limit = value; return self
    def unlimited(self): self._limit = None; return self
    def precision(self, value): self._precision = value; return self
    def order_by(self, *expressions): self._orders.extend(expressions); return self
    def output(self, *expressions): self._outputs.extend(e if isinstance(e, _Output) else e.as_("") for e in expressions); return self
    def subgroup(self, group): self._subgroups.append(group); return self
    def alias(self, name, expression): self._aliases.append((name, expression)); return self
    def keep(self, expression): self._keep = expression; return self
    def filter(self, expression): self._filter = expression; return self

    def _body_operations(self):
        operations = [
            GroupingOperationT(
                operationType=GroupingOperationKind.GroupingAliasOperation,
                operation=GroupingAliasOperationT(name=n, expression=e.model),
            )
            for n, e in self._aliases
        ]
        if self._limit is not None:
            operations.append(GroupingOperationT(operationType=GroupingOperationKind.GroupingMaxOperation, operation=GroupingMaxOperationT(valueType=GroupingMaxKind.FiniteGroupingMax, value=FiniteGroupingMaxT(value=self._limit))))
        if self._precision is not None:
            operations.append(GroupingOperationT(operationType=GroupingOperationKind.GroupingPrecisionOperation, operation=GroupingPrecisionOperationT(value=self._precision)))
        if self._orders:
            operations.append(GroupingOperationT(operationType=GroupingOperationKind.GroupingOrderOperation, operation=GroupingOrderOperationT(expressions=[GroupingOrderExprT(expression=o.expression.model) for o in self._orders])))
        if self._filter is not None:
            operations.append(GroupingOperationT(operationType=GroupingOperationKind.GroupingFilterOperation, operation=__import__("SearchKernel.V2.GroupingFilterOperation", fromlist=["GroupingFilterOperationT"]).GroupingFilterOperationT(filter=self._filter)))
        if self._keep is not None:
            operations.append(GroupingOperationT(operationType=GroupingOperationKind.GroupingKeepOperation, operation=GroupingKeepOperationT(filter=self._keep)))
        if self._outputs:
            operations.append(GroupingOperationT(operationType=GroupingOperationKind.GroupingOutputOperation, operation=GroupingOutputOperationT(outputs=[GroupingOutputT(label=o.label or None, expression=o.expression.model) for o in self._outputs])))
        for subgroup in self._subgroups:
            operations.append(GroupingOperationT(
                operationType=GroupingOperationKind.GroupingAllOperation,
                operation=GroupingAllOperationT(operations=subgroup._operations()),
            ))
        return operations

    def _operations(self):
        return [
            GroupingOperationT(
                operationType=GroupingOperationKind.GroupingGroupOperation,
                operation=GroupingGroupOperationT(expression=self.expression.model),
            ),
            GroupingOperationT(
                operationType=GroupingOperationKind.GroupingEachOperation,
                operation=GroupingEachOperationT(operations=self._body_operations()),
            ),
        ]

    def to_request(self):
        root = GroupingAllOperationT(operations=self._operations())
        return GroupingRequestT(sourceType=GroupingSourceTags.StructuredGroupingSource, source=StructuredGroupingSourceT(root=root))


def _facet_request(field: str, limit: int, ordering: str | None):
    ordering = ordering or "count_desc"
    if ordering == "count_desc":
        expression = -agg.count()
    elif ordering == "count_asc":
        expression = agg.count()
    elif ordering == "value_desc":
        expression = -g.field(field)
    elif ordering == "value_asc":
        expression = g.field(field)
    else:
        raise ValueError("facet sort must be count_desc, count_asc, value_asc, or value_desc")
    each = GroupingEachOperationT(operations=[
        GroupingOperationT(
            operationType=GroupingOperationKind.GroupingMaxOperation,
            operation=GroupingMaxOperationT(
                valueType=GroupingMaxKind.FiniteGroupingMax,
                value=FiniteGroupingMaxT(value=limit),
            ),
        ),
        GroupingOperationT(
            operationType=GroupingOperationKind.GroupingOrderOperation,
            operation=GroupingOrderOperationT(expressions=[GroupingOrderExprT(expression=expression.model)]),
        ),
        GroupingOperationT(
            operationType=GroupingOperationKind.GroupingOutputOperation,
            operation=GroupingOutputOperationT(outputs=[GroupingOutputT(expression=agg.count().model)]),
        ),
    ])
    root = GroupingAllOperationT(operations=[
        GroupingOperationT(
            operationType=GroupingOperationKind.GroupingGroupOperation,
            operation=GroupingGroupOperationT(expression=g.field(field).model),
        ),
        GroupingOperationT(operationType=GroupingOperationKind.GroupingEachOperation, operation=each),
    ])
    return GroupingRequestT(
        sourceType=GroupingSourceTags.StructuredGroupingSource,
        source=StructuredGroupingSourceT(root=root),
    )


def _root_aggregation_request(outputs):
    root = GroupingAllOperationT(operations=[
        GroupingOperationT(
            operationType=GroupingOperationKind.GroupingOutputOperation,
            operation=GroupingOutputOperationT(
                outputs=[GroupingOutputT(label=o.label or None, expression=o.expression.model) for o in outputs]
            ),
        )
    ])
    return GroupingRequestT(
        sourceType=GroupingSourceTags.StructuredGroupingSource,
        source=StructuredGroupingSourceT(root=root),
    )


class SearchBuilder:
    def __init__(self, index, query):
        self.index, self.query = index, query
        self._filters, self._sort, self._fields, self._groups = [], [], None, []
        self._summary = None
        self._highlighting = None
        self._aggregates, self._facets = [], []
        self._ranking, self._rank_properties, self._inputs, self._properties = None, {}, {}, {}
        self._limit = self._offset = None
        self._paged = False

    def filter(self, source, bindings=None): self._filters.append((source, dict(bindings or {}))); return self
    def sort(self, field, direction="asc"):
        if direction not in {"asc", "desc"}: raise ValueError("sort direction must be 'asc' or 'desc'")
        self._sort.append(SortExprT(target=SortTargetT(targetType=SortTargetKind.FieldSortTarget, target=FieldSortTargetT(field=field)), direction=SortDirection.Asc if direction == "asc" else SortDirection.Desc)); return self
    def fields(self, *fields): self._fields = list(fields); return self
    def summary(self, name): self._summary = name; return self
    def highlight(self, *fields): self._highlighting = list(fields); return self
    def facet(self, field, *, name=None, limit=None, sort=None): self._facets.append((field, name or field, limit, sort)); return self
    def group(self, group): self._groups.append(group); return self
    def aggregate(self, expression): self._aggregates.append(expression if isinstance(expression, _Output) else expression.as_("")); return self
    def ranking(self, profile): self._ranking = profile; return self
    def input(self, name, item): self._inputs[name] = item; return self
    def rank_property(self, name, item): self._rank_properties[name] = str(item); return self
    def property(self, name, item): self._properties[name] = value(item); return self
    def limit(self, item):
        if item < 0 or self._paged: raise ValueError("limit must be non-negative and cannot follow page()")
        self._limit = item; return self
    def offset(self, item):
        if item < 0 or self._paged: raise ValueError("offset must be non-negative and cannot follow page()")
        self._offset = item; return self
    def page(self, number, size):
        if number < 1 or size < 0 or self._limit is not None or self._offset is not None: raise ValueError("invalid page() combination")
        self._paged, self._offset, self._limit = True, (number - 1) * size, size; return self

    def execute(self):
        textual = [
            FilterClauseT(
                source=source,
                bindings=[
                    ValueEntryT(key=name, value=value(item))
                    for name, item in sorted(bindings.items())
                ],
            )
            for source, bindings in self._filters
        ]
        filters = FilterRequestT(
            textual=textual,
        ) if self._filters else None
        groups = [group.to_request() for group in self._groups]
        for field, name, limit, _sort in self._facets:
            groups.append(_facet_request(field, limit or 10, _sort))
        if self._aggregates:
            groups.append(_root_aggregation_request(self._aggregates))
        query = self.query
        if query is None: query = QuerySourceT(sourceType=QuerySourceKind.MatchAllQuery, source=MatchAllQueryT())
        elif isinstance(query, str): query = QuerySourceT(sourceType=QuerySourceKind.SearchKernelTextQuery, source=SearchKernelTextQueryT(text=query))
        inputs = [QueryInputT(name=name, inputType=QueryInputKind.LiteralQueryInput, input=LiteralQueryInputT(value=value(item))) for name, item in sorted(self._inputs.items())]
        ranking = RankingRequestT(profile=self._ranking or "default", properties=[StringEntryT(key=k, value=v) for k, v in sorted(self._rank_properties.items())]) if self._ranking is not None or self._rank_properties else None
        presentation = PresentationRequestT(
            summary=self._summary,
            fields=FieldProjectionT(fields=self._fields) if self._fields is not None else None,
            highlighting=HighlightRequestT(fields=self._highlighting)
            if self._highlighting is not None
            else None,
        ) if self._summary is not None or self._fields is not None or self._highlighting is not None else None
        request = SearchRequestT(query=query, filters=filters, sort=self._sort, grouping=groups, ranking=ranking, inputs=inputs, limit=OptionalUInt32T(value=self._limit) if self._limit is not None else None, offset=OptionalUInt32T(value=self._offset) if self._offset is not None else None, presentation=presentation, properties=[ValueEntryT(key=k, value=v) for k, v in sorted(self._properties.items())])
        results = SearchResults(self.index.client.search(request))
        for field, name, _limit, _sort in self._facets:
            if name != field and field in results.facets:
                results.facets[name] = results.facets[field]
        return results


class YqlBuilder(SearchBuilder):
    def __init__(self, index, source):
        super().__init__(index, QuerySourceT(sourceType=QuerySourceKind.YqlQuery, source=YqlQueryT(source=source)))


class PatchBuilder:
    def __init__(self, index, identifier): self.index, self.identifier, self.operations = index, identifier, []
    def set(self, field, item): self.operations.append(PatchOperationT(operationType=PatchOperationKind.SetField, operation=SetFieldT(field=field, value=value(item)))); return self
    def unset(self, field): self.operations.append(PatchOperationT(operationType=PatchOperationKind.UnsetField, operation=UnsetFieldT(field=field))); return self
    def execute(self): return self.index.client.patch(PatchRequestT(id=self.identifier, operations=self.operations))


class UpdateBuilder:
    def __init__(self, index, identifier):
        self.index, self.identifier = index, identifier
        self.operations = []
        self._condition = None
        self._create = None

    def set(self, field, item):
        self.operations.append(UpdateOperationT(
            target=FieldPathT(path=field),
            operationType=UpdateOperationKind.AssignUpdate,
            operation=AssignUpdateT(value=value(item)),
        ))
        return self

    assign = set

    def unset(self, field):
        self.operations.append(UpdateOperationT(
            target=FieldPathT(path=field),
            operationType=UpdateOperationKind.ClearUpdate,
            operation=ClearUpdateT(),
        ))
        return self

    clear = unset

    def _arithmetic(self, field, item, operator):
        self.operations.append(UpdateOperationT(
            target=FieldPathT(path=field),
            operationType=UpdateOperationKind.ArithmeticUpdate,
            operation=ArithmeticUpdateT(operator=operator, operand=value(item)),
        ))
        return self

    def increment(self, field, item):
        return self._arithmetic(field, item, ArithmeticUpdateOperator.Increment)

    def decrement(self, field, item):
        return self._arithmetic(field, item, ArithmeticUpdateOperator.Decrement)

    def multiply(self, field, item):
        return self._arithmetic(field, item, ArithmeticUpdateOperator.Multiply)

    def divide(self, field, item):
        return self._arithmetic(field, item, ArithmeticUpdateOperator.Divide)

    def condition(self, expression):
        self._condition = expression
        return self

    def create(self, enabled=True):
        self._create = enabled
        return self

    def execute(self):
        request = UpdateRequestT(
            id=self.identifier,
            operations=self.operations,
            condition=self._condition,
            create=OptionalBoolT(value=self._create) if self._create is not None else None,
        )
        return self.index.client.update(request)


def _value_entries(entries) -> dict[str, Any]:
    return {
        entry.key: _python_value(entry.value)
        for entry in (entries or [])
        if entry is not None and entry.key is not None
    }


def _feature_entries(entries) -> dict[str, Any]:
    converted = _value_entries(entries)
    for name, item in converted.items():
        if isinstance(item, list) and all(
            isinstance(cell, int) and 0 <= cell <= 255 for cell in item
        ):
            converted[name] = bytes(item)
    return converted


def _optional_value(item):
    return getattr(item, "value", None) if item is not None else None


class CoverageDegraded:
    def __init__(self, node):
        self.match_phase = _optional_value(getattr(node, "matchPhase", None))
        self.timeout = _optional_value(getattr(node, "timeout", None))
        self.adaptive_timeout = _optional_value(getattr(node, "adaptiveTimeout", None))
        self.ann_timeout = _optional_value(getattr(node, "annTimeout", None))
        self.non_ideal_state = _optional_value(getattr(node, "nonIdealState", None))
        self.metadata = _value_entries(getattr(node, "metadata", None))


class Coverage:
    def __init__(self, node):
        self.coverage_percent = _optional_value(getattr(node, "coveragePercent", None))
        self.documents = _optional_value(getattr(node, "documents", None))
        self.full = _optional_value(getattr(node, "full", None))
        self.nodes = _optional_value(getattr(node, "nodes", None))
        self.results = _optional_value(getattr(node, "results", None))
        self.results_full = _optional_value(getattr(node, "resultsFull", None))
        degraded = getattr(node, "degraded", None)
        self.degraded = CoverageDegraded(degraded) if degraded is not None else None
        self.metadata = _value_entries(getattr(node, "metadata", None))


class Hit(dict):
    def __init__(self, node):
        super().__init__(_value_entries(node.fields))
        self.id, self.fields = node.id or "", self
        relevance = node.relevance.value if node.relevance is not None else None
        self.relevance = getattr(relevance, "value", relevance)
        self.score = self.relevance
        self.match_features = _feature_entries(getattr(node, "matchFeatures", None))
        self.summary_features = _feature_entries(getattr(node, "summaryFeatures", None))
        self.rank_features = _feature_entries(getattr(node, "rankFeatures", None))
        self.features = self.match_features
        self.sort_values = [
            _python_value(item) for item in (getattr(node, "sortValues", None) or [])
        ]
        self.highlights = [
            {
                "field": highlight.field or "",
                "fragments": [
                    {
                        "text": fragment.text or "",
                        "matches": [
                            (match.startByte, match.endByte)
                            for match in (fragment.matches or [])
                        ],
                    }
                    for fragment in (highlight.fragments or [])
                ],
            }
            for highlight in (getattr(node, "highlights", None) or [])
        ]


class ResultGroup(dict):
    def __init__(self, node):
        super().__init__({entry.key: _python_value(entry.value) for entry in (node.fields or [])})
        self.value = _python_value(node.value)
        self.label, self.fields = node.label, self
        self.children = [_node(child) for child in (node.children or [])]
        self.groups = [child for child in self.children if isinstance(child, ResultGroup)]
        self.hits = [child for child in self.children if isinstance(child, Hit)]
        self.continuations = [entry.value for entry in (node.continuations or [])]


def _node(node):
    return ResultGroup(node.node) if node.nodeType == ResultNodeKind.ResultGroup else Hit(node.node)


class SearchResults:
    def __init__(self, result):
        self._result = result
        self.root = _node(result.root) if result.root is not None else None
        children = self.root.children if isinstance(self.root, ResultGroup) else []
        self.hits = [child for child in children if isinstance(child, Hit)]
        self.groups = []
        for child in children:
            if (
                isinstance(child, ResultGroup)
                and child.value is None
                and child.label is None
                and child.groups
            ):
                # The canonical result keeps a grouping node beneath the
                # search root. The ergonomic grouping projection exposes its
                # buckets directly while retaining the canonical tree on
                # `root`.
                self.groups.extend(child.groups)
            elif isinstance(child, ResultGroup):
                self.groups.append(child)
        self.total_hits = result.totalHits.value if result.totalHits is not None else 0
        self.total_hits_relation = result.totalHits.relation if result.totalHits is not None else "unknown"
        coverage = getattr(result, "coverage", None)
        self.coverage = Coverage(coverage) if coverage is not None else None
        self.issues = list(getattr(result, "errors", None) or [])
        self.aggregations = dict(self.root) if isinstance(self.root, ResultGroup) else {}
        self.continuations = list(getattr(self.root, "continuations", []) or []) if self.root else []
        self.facets = {}
        for group in self.groups:
            if group.value is not None or not group.label or not group.groups:
                continue
            for child in group.groups:
                self.facets.setdefault(group.label, []).append(
                    {"value": child.value, "count": child.get("count", 0)}
                )


__all__ = [
    "Boolean", "Coverage", "CoverageDegraded", "Float", "Group", "Hit",
    "Integer", "Keyword", "PatchBuilder", "RankProfile", "ResultGroup",
    "SearchBuilder", "SearchResults", "Tensor", "TensorField", "Text",
    "UpdateBuilder", "YqlBuilder", "agg", "asc", "desc", "g", "gf", "q",
]
