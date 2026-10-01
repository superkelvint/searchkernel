"""Idiomatic Python builders for the Search Kernel portable API."""

from __future__ import annotations

from dataclasses import dataclass
from typing import Any, Mapping
from ._semantic import is_semantic_table, table_type

from ._semantic import AggregationKind
from ._semantic import AvgAggregationT
from ._semantic import CountAggregationT
from ._semantic import FieldDefinitionT
from ._semantic import DataTypeT
from ._semantic import DataTypeKind
from ._semantic import TensorDataTypeT
from ._semantic import ArrayDataTypeT
from ._semantic import StructDataTypeT
from ._semantic import StructFieldDefinitionT
from ._semantic import PositionDataTypeT
from ._semantic import RankPhaseDefinitionT
from ._semantic import RankProfileDefinitionT
from ._semantic import RankProfileInputT
from ._semantic import GroupingBinaryOperator
from ._semantic import FilterClauseT
from ._semantic import FilterRequestT
from ._semantic import GroupingAggregationExpressionT
from ._semantic import GroupingAliasOperationT
from ._semantic import GroupingAliasReferenceT
from ._semantic import GroupingAllOperationT
from ._semantic import GroupingAttributeExpressionT
from ._semantic import GroupingBinaryExpressionT
from ._semantic import GroupingEachOperationT
from ._semantic import GroupingExpressionT
from ._semantic import GroupingExpressionKind
from ._semantic import GroupingFilterAndT
from ._semantic import GroupingFilterExprT
from ._semantic import GroupingFilterExprKind
from ._semantic import GroupingFilterIsTrueT
from ._semantic import GroupingFilterNotT
from ._semantic import GroupingFilterOrT
from ._semantic import GroupingFilterRegexT
from ._semantic import GroupingFunctionExpressionT
from ._semantic import GroupingGroupOperationT
from ._semantic import GroupingKeepOperationT
from ._semantic import GroupingLiteralExpressionT
from ._semantic import GroupingMaxKind
from ._semantic import GroupingMaxOperationT
from ._semantic import GroupingOperationT
from ._semantic import GroupingOperationKind
from ._semantic import GroupingOrderExprT
from ._semantic import GroupingOrderOperationT
from ._semantic import GroupingOutputT
from ._semantic import GroupingOutputOperationT
from ._semantic import GroupingPrecisionOperationT
from ._semantic import GroupingRequestT
from ._semantic import GroupingSourceKind
from ._semantic import GroupingSourceKind as GroupingSourceTags
from ._semantic import GroupingUnaryExpressionT
from ._semantic import GroupingUnaryOperator
from ._semantic import FiniteGroupingMaxT
from ._semantic import StructuredGroupingSourceT
from ._semantic import PresentationRequestT
from ._semantic import FieldProjectionT
from ._semantic import HighlightRequestT
from ._semantic import QueryAndT
from ._semantic import QueryContainsT
from ._semantic import QueryExprT
from ._semantic import QueryExprKind
from ._semantic import QueryFuzzyT
from ._semantic import QueryGeoBoundingBoxT
from ._semantic import QueryGeoLocationT
from ._semantic import QueryMatchesT
from ._semantic import QueryFieldRefT
from ._semantic import QueryNearT
from ._semantic import QueryNearestNeighborT
from ._semantic import QueryNotT
from ._semantic import QueryONearT
from ._semantic import QueryOrT
from ._semantic import QueryPhraseT
from ._semantic import QueryRankT
from ._semantic import QuerySameElementT
from ._semantic import QueryInputT
from ._semantic import QueryInputKind
from ._semantic import LiteralQueryInputT
from ._semantic import ExpressionQueryInputT
from ._semantic import MatchingMode
from ._semantic import PrimitiveDataTypeKind
from ._semantic import QuerySourceT
from ._semantic import QuerySourceKind
from ._semantic import SearchFieldT
from ._semantic import SearchFieldSelectionT
from ._semantic import SearchKernelTextQueryT
from ._semantic import MatchAllQueryT
from ._semantic import YqlQueryT
from ._semantic import TypedQueryT
from ._semantic import QueryWandT
from ._semantic import QueryComparisonT
from ._semantic import ComparisonOp
from ._semantic import QueryRangeT
from ._semantic import QueryInT
from ._semantic import RangeBoundT
from ._semantic import RangeBoundKind
from ._semantic import FiniteRangeBoundT
from ._semantic import QueryWeightedSetT
from ._semantic import QueryDotProductT
from ._semantic import WeightedQueryItemT
from ._semantic import OptionalBoolT
from ._semantic import QueryWeakAndT
from ._semantic import WeightedQueryItemT
from ._semantic import RankingRequestT
from ._semantic import RelevanceKind
from ._semantic import FiniteRelevanceT
from ._semantic import PositiveInfinityRelevanceT
from ._semantic import NegativeInfinityRelevanceT
from ._semantic import SearchRequestT
from ._semantic import OptionalUInt32T
from ._semantic import SortDirection
from ._semantic import SortExprT
from ._semantic import SortTargetT
from ._semantic import SortTargetKind
from ._semantic import FieldSortTargetT
from ._semantic import StringEntryT
from ._semantic import ValueEntryT
from ._semantic import ValueT
from ._semantic import SetFieldT
from ._semantic import UnsetFieldT
from ._semantic import PatchOperationT
from ._semantic import PatchOperationKind
from ._semantic import PatchRequestT
from ._semantic import ArithmeticUpdateT
from ._semantic import ArithmeticUpdateOperator
from ._semantic import AssignUpdateT
from ._semantic import ClearUpdateT
from ._semantic import FieldPathT
from ._semantic import OptionalBoolT
from ._semantic import OptionalUInt32T
from ._semantic import AttributeConfigT
from ._semantic import DistanceMetric
from ._semantic import HnswIndexConfigT
from ._semantic import IndexConfigT
from ._semantic import UpdateOperationT
from ._semantic import UpdateOperationKind
from ._semantic import UpdateRequestT
from ._semantic import ResultNodeKind
from ._semantic import ValueKind
from ._semantic import HitT
from ._semantic import ResultGroupT

from .client import Tensor, _decode_value_entries, _python_value, value
from .scalars import check_uint32


_MISSING = object()
_REGEX_LITERAL_SPECIALS = frozenset(".^$*+?()[]{}|\\")


def _literal_regex(value: str) -> str:
    return "".join(
        ("\\" + character) if character in _REGEX_LITERAL_SPECIALS else character
        for character in value
    )


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
        optional = lambda setting: None if setting is None else table_type("OptionalBool")(value=setting)
        return FieldDefinitionT(
            name=name,
            dataType=DataTypeT(kindType=DataTypeKind.PrimitiveDataType, kind=table_type("PrimitiveDataType")(kind=primitive)),
            stored=optional(self.stored), searchable=optional(self.searchable),
            filterable=optional(self.filterable), sortable=optional(self.sortable),
            facetable=optional(self.facetable),
            matching=(
                table_type("MatchingConfig")(
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
        optional = lambda setting, label: None if setting is None else OptionalUInt32T(
            value=check_uint32(setting, label)
        )
        return HnswIndexConfigT(
            maxLinksPerNode=optional(self.max_links_per_node, "max_links_per_node"),
            neighborsToExploreAtInsert=optional(
                self.neighbors_to_explore_at_insert, "neighbors_to_explore_at_insert"
            ),
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
        optional = lambda setting: None if setting is None else table_type("OptionalBool")(value=setting)
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


class PositionField(_FieldSpec):
    def definition(self, name: str) -> FieldDefinitionT:
        return FieldDefinitionT(
            name=name,
            dataType=DataTypeT(kindType=DataTypeKind.PositionDataType, kind=PositionDataTypeT()),
            stored=None if self.stored is None else table_type("OptionalBool")(value=self.stored),
            searchable=None if self.searchable is None else table_type("OptionalBool")(value=self.searchable),
            filterable=None if self.filterable is None else table_type("OptionalBool")(value=self.filterable),
            sortable=None if self.sortable is None else table_type("OptionalBool")(value=self.sortable),
            facetable=None if self.facetable is None else table_type("OptionalBool")(value=self.facetable),
        )


class StructField(_FieldSpec):
    def __init__(self, fields: Mapping[str, _FieldSpec], **kwargs):
        super().__init__(**kwargs)
        self.fields = fields

    def definition(self, name: str) -> FieldDefinitionT:
        members = []
        for member_name, member in sorted(self.fields.items()):
            nested = member.definition(member_name).dataType
            members.append(StructFieldDefinitionT(name=member_name, dataType=nested))
        data_type = DataTypeT(kindType=DataTypeKind.StructDataType, kind=StructDataTypeT(fields=members))
        optional = lambda setting: None if setting is None else OptionalBoolT(value=setting)
        return FieldDefinitionT(
            name=name,
            dataType=data_type,
            stored=optional(self.stored),
            searchable=optional(self.searchable),
            filterable=optional(self.filterable),
            sortable=optional(self.sortable),
            facetable=optional(self.facetable),
        )


class ArrayField(_FieldSpec):
    def __init__(self, element: _FieldSpec, **kwargs):
        super().__init__(**kwargs)
        self.element = element

    def definition(self, name: str) -> FieldDefinitionT:
        element = self.element.definition("element").dataType
        return FieldDefinitionT(
            name=name,
            dataType=DataTypeT(kindType=DataTypeKind.ArrayDataType, kind=ArrayDataTypeT(elementType=element)),
            stored=None if self.stored is None else table_type("OptionalBool")(value=self.stored),
            searchable=None if self.searchable is None else table_type("OptionalBool")(value=self.searchable),
            filterable=None if self.filterable is None else table_type("OptionalBool")(value=self.filterable),
            sortable=None if self.sortable is None else table_type("OptionalBool")(value=self.sortable),
            facetable=None if self.facetable is None else table_type("OptionalBool")(value=self.facetable),
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
        self._rerank_count = check_uint32(count, "rerank count")
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
        GroupingMemberExpressionT = table_type("GroupingMemberExpression")
        return _GroupingExpr(_expr_union(GroupingExpressionKind.GroupingMemberExpression, GroupingMemberExpressionT(base=_as_expr(base).model, member=name)))

    def lookup(self, base: Any, key: Any) -> _GroupingExpr:
        GroupingMapLookupExpressionT = table_type("GroupingMapLookupExpression")
        return _GroupingExpr(_expr_union(GroupingExpressionKind.GroupingMapLookupExpression, GroupingMapLookupExpressionT(base=_as_expr(base).model, key=_as_expr(key).model)))


g = _GroupingNamespace()


class _AggregationNamespace:
    def _make(self, kind: str, field: str | None = None, summary: str | None = None) -> _GroupingExpr:
        payloads = {
            "count": (AggregationKind.CountAggregation, CountAggregationT()),
            "sum": (AggregationKind.SumAggregation, table_type("SumAggregation")(expression=g.field(field).model)),
            "avg": (AggregationKind.AvgAggregation, AvgAggregationT(expression=g.field(field).model)),
            "min": (AggregationKind.MinAggregation, table_type("MinAggregation")(expression=g.field(field).model)),
            "max": (AggregationKind.MaxAggregation, table_type("MaxAggregation")(expression=g.field(field).model)),
            "stddev": (AggregationKind.StdDevAggregation, table_type("StdDevAggregation")(expression=g.field(field).model)),
            "summary": (AggregationKind.SummaryAggregation, table_type("SummaryAggregation")(summary=summary)),
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
        if is_semantic_table(source, "QueryExpr"):
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
            expr=table_type("TermQuery")(
                value=item if is_semantic_table(item, "Value") else value(item)
            ),
        )

    def term(self, field_or_item: Any, item: Any = _MISSING) -> QueryExprT | QuerySourceT:
        if item is _MISSING:
            return self._term(field_or_item)
        return self.contains(str(field_or_item), self._term(item))

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
            expr=QueryFuzzyT(value=item if is_semantic_table(item, "Value") else value(item)),
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

    def equals(self, field: str, item: Any) -> QuerySourceT:
        return self._combine(QueryExprKind.QueryComparison, QueryComparisonT(
            field=QueryFieldRefT(path=field), op=ComparisonOp.Eq, value=value(item)))

    def in_values(self, field: str, items: list[Any]) -> QuerySourceT:
        if not items:
            raise ValueError("q.in_values requires at least one value")
        return self._combine(QueryExprKind.QueryIn, QueryInT(
            field=QueryFieldRefT(path=field), values=[value(item) for item in items]))

    def range(
        self,
        field: str,
        lower: Any,
        upper: Any,
        *,
        include_lower: bool = True,
        include_upper: bool = True,
    ) -> QuerySourceT:
        def bound(item):
            return RangeBoundT(
                boundType=RangeBoundKind.FiniteRangeBound,
                bound=FiniteRangeBoundT(value=value(item)),
            )
        return self._combine(QueryExprKind.QueryRange, QueryRangeT(
            field=QueryFieldRefT(path=field), lower=bound(lower), upper=bound(upper),
            includeLower=OptionalBoolT(value=include_lower),
            includeUpper=OptionalBoolT(value=include_upper)))

    def matches(self, field: str, pattern: str) -> QuerySourceT:
        return self._combine(
            QueryExprKind.QueryMatches,
            QueryMatchesT(field=QueryFieldRefT(path=field), pattern=pattern),
        )

    def prefix(self, field: str, item: str) -> QuerySourceT:
        return self.matches(field, _literal_regex(item) + ".*")

    def substring(self, field: str, item: str) -> QuerySourceT:
        return self.matches(field, ".*" + _literal_regex(item) + ".*")

    def suffix(self, field: str, item: str) -> QuerySourceT:
        return self.matches(field, ".*" + _literal_regex(item))

    def weighted_set(self, field: str, items: Mapping[Any, int]) -> QuerySourceT:
        weighted = [WeightedQueryItemT(key=value(key), weight=weight)
                    for key, weight in sorted(items.items(), key=lambda item: str(item[0]))]
        return self._combine(QueryExprKind.QueryWeightedSet,
                             QueryWeightedSetT(field=QueryFieldRefT(path=field), items=weighted))

    def dot_product(self, field: str, items: Mapping[Any, int]) -> QuerySourceT:
        weighted = [WeightedQueryItemT(key=value(key), weight=weight)
                    for key, weight in sorted(items.items(), key=lambda item: str(item[0]))]
        return self._combine(QueryExprKind.QueryDotProduct,
                             QueryDotProductT(field=QueryFieldRefT(path=field), items=weighted))

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
    def limit(self, value): self._limit = check_uint32(value, "grouping limit"); return self
    def unlimited(self): self._limit = None; return self
    def precision(self, value): self._precision = check_uint32(value, "grouping precision"); return self
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
            operations.append(GroupingOperationT(operationType=GroupingOperationKind.GroupingFilterOperation, operation=table_type("GroupingFilterOperation")(filter=self._filter)))
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


def _facet_request(field: str, name: str, limit: int, ordering: str | None):
    limit = check_uint32(limit, "facet limit")
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
    ], label=name)
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


def _uint32(item, label):
    return check_uint32(item, label)


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
    def sort(self, field, direction):
        if direction not in {"asc", "desc"}: raise ValueError("sort direction must be 'asc' or 'desc'")
        self._sort.append(SortExprT(target=SortTargetT(targetType=SortTargetKind.FieldSortTarget, target=FieldSortTargetT(field=field)), direction=SortDirection.Asc if direction == "asc" else SortDirection.Desc)); return self
    def fields(self, *fields): self._fields = list(fields); return self
    def summary(self, name): self._summary = name; return self
    def highlight(self, *fields): self._highlighting = list(fields); return self
    def facet(self, field, *, name=None, limit=None, sort=None):
        if limit is not None:
            check_uint32(limit, "facet limit")
        self._facets.append((field, name or field, limit, sort))
        return self
    def group(self, group): self._groups.append(group); return self
    def aggregate(self, expression): self._aggregates.append(expression if isinstance(expression, _Output) else expression.as_("")); return self
    def ranking(self, profile): self._ranking = profile; return self
    def input(self, name, item): self._inputs[name] = item; return self
    def rank_property(self, name, item: str):
        if not isinstance(item, str):
            raise TypeError("rank property value must be a string")
        self._rank_properties[name] = item
        return self
    def property(self, name, item): self._properties[name] = value(item); return self
    def limit(self, item):
        _uint32(item, "limit")
        if self._paged: raise ValueError("limit must be non-negative and cannot follow page()")
        self._limit = item; return self
    def offset(self, item):
        _uint32(item, "offset")
        if self._paged: raise ValueError("offset must be non-negative and cannot follow page()")
        self._offset = item; return self
    def page(self, number, size):
        if (
            isinstance(number, bool)
            or not isinstance(number, int)
            or number < 1
            or self._limit is not None
            or self._offset is not None
        ):
            raise ValueError("invalid page() combination")
        _uint32(size, "page size")
        if number * size > 0xFFFF_FFFF:
            raise ValueError("page range exceeds uint32 range")
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
            groups.append(_facet_request(field, name, 10 if limit is None else limit, _sort))
        if self._aggregates:
            groups.append(_root_aggregation_request(self._aggregates))
        query = self.query
        if query is None: query = QuerySourceT(sourceType=QuerySourceKind.MatchAllQuery, source=MatchAllQueryT())
        elif isinstance(query, str): query = QuerySourceT(sourceType=QuerySourceKind.SearchKernelTextQuery, source=SearchKernelTextQueryT(text=query))
        inputs = [QueryInputT(name=name, inputType=QueryInputKind.LiteralQueryInput, input=LiteralQueryInputT(value=value(item))) for name, item in sorted(self._inputs.items())]
        ranking = RankingRequestT(profile=self._ranking, properties=[StringEntryT(key=k, value=v) for k, v in sorted(self._rank_properties.items())]) if self._ranking is not None or self._rank_properties else None
        presentation = PresentationRequestT(
            summary=self._summary,
            fields=FieldProjectionT(fields=self._fields) if self._fields is not None else None,
            highlighting=HighlightRequestT(fields=self._highlighting)
            if self._highlighting is not None
            else None,
        ) if self._summary is not None or self._fields is not None or self._highlighting is not None else None
        request = SearchRequestT(query=query, filters=filters, sort=self._sort, grouping=groups, ranking=ranking, inputs=inputs, limit=OptionalUInt32T(value=self._limit) if self._limit is not None else None, offset=OptionalUInt32T(value=self._offset) if self._offset is not None else None, presentation=presentation, properties=[ValueEntryT(key=k, value=v) for k, v in sorted(self._properties.items())])
        results = SearchResults(self.index.client.search(request))
        for _field, name, _limit, _sort in self._facets:
            results._project_facet(name, name, _field)
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
    return _decode_value_entries(entries)


def _feature_entries(entries) -> dict[str, Any]:
    return _value_entries(entries)


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


class SearchTiming:
    def __init__(self, node):
        self.query_time_seconds = _optional_value(getattr(node, "queryTimeSeconds", None))
        self.search_time_seconds = _optional_value(getattr(node, "searchTimeSeconds", None))
        self.summary_fetch_time_seconds = _optional_value(
            getattr(node, "summaryFetchTimeSeconds", None)
        )
        self.metadata = _value_entries(getattr(node, "metadata", None))


class ResultIssue:
    def __init__(self, node):
        self.code = node.code
        self.summary = node.summary
        self.message = node.message
        self.source = node.source
        self.stack_trace = node.stackTrace
        self.transient = _optional_value(getattr(node, "transient", None))
        self.metadata = _value_entries(getattr(node, "metadata", None))


class ResultLimits:
    def __init__(self, node):
        self.from_ = _python_value(getattr(node, "from_", None))
        self.to = _python_value(getattr(node, "to", None))


class ResultContinuation:
    def __init__(self, node):
        if not isinstance(node.key, str) or not isinstance(node.value, str):
            raise ValueError("result continuation has a missing key or value")
        self.key = node.key
        self.value = node.value


def _relevance_value(node):
    if node is None or node.valueType == RelevanceKind.NONE:
        return None
    if node.valueType == RelevanceKind.FiniteRelevance:
        if not is_semantic_table(node.value, "FiniteRelevance"):
            raise ValueError("finite relevance has a missing or mismatched payload")
        return node.value.value
    if node.valueType == RelevanceKind.PositiveInfinityRelevance:
        if not is_semantic_table(node.value, "PositiveInfinityRelevance"):
            raise ValueError("positive-infinity relevance has a missing or mismatched payload")
        return float("inf")
    if node.valueType == RelevanceKind.NegativeInfinityRelevance:
        if not is_semantic_table(node.value, "NegativeInfinityRelevance"):
            raise ValueError("negative-infinity relevance has a missing or mismatched payload")
        return float("-inf")
    raise ValueError(f"unsupported relevance kind: {node.valueType}")


class Hit(dict):
    def __init__(self, node):
        super().__init__(_value_entries(node.fields))
        self.id, self.fields = node.id or "", self
        self.relevance = _relevance_value(getattr(node, "relevance", None))
        self.score = self.relevance
        self.source = getattr(node, "source", None)
        self.types = [
            item.decode("utf-8") if isinstance(item, bytes) else item
            for item in (getattr(node, "types", None) or [])
        ]
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
        self._field_entries = list(node.fields or [])
        super().__init__(_value_entries(self._field_entries))
        self.id = node.id
        self.source = node.source
        self.types = [
            item.decode("utf-8") if isinstance(item, bytes) else item
            for item in (node.types or [])
        ]
        self.value = _python_value(node.value)
        self.relevance = _relevance_value(node.relevance)
        self.limits = ResultLimits(node.limits) if node.limits is not None else None
        self.metadata = _value_entries(node.metadata)
        self.label, self.fields = node.label, self
        self.children = [_node(child) for child in (node.children or [])]
        self.groups = [child for child in self.children if isinstance(child, ResultGroup)]
        self.hits = [child for child in self.children if isinstance(child, Hit)]
        self.continuations = [ResultContinuation(entry) for entry in (node.continuations or [])]


def _node(node):
    if node is None or getattr(node, "node", None) is None:
        raise ValueError("missing ResultNode payload")
    node_type = getattr(node, "nodeType", ResultNodeKind.NONE)
    if node_type == ResultNodeKind.ResultGroup:
        if not is_semantic_table(node.node, "ResultGroup"):
            raise ValueError("mismatched ResultNode union: ResultGroup payload")
        return ResultGroup(node.node)
    if node_type == ResultNodeKind.Hit:
        if not is_semantic_table(node.node, "Hit"):
            raise ValueError("mismatched ResultNode union: Hit payload")
        return Hit(node.node)
    raise ValueError(f"unknown ResultNode union kind: {node_type}")


class SearchResults:
    def __init__(self, result):
        self._result = result
        if result.root is None:
            raise ValueError("missing SearchResult.root")
        if result.totalHits is None:
            raise ValueError("missing SearchResult.total_hits")
        self.root = _node(result.root)
        if isinstance(self.root, ResultGroup):
            children = self.root.children
            self.hits = [child for child in children if isinstance(child, Hit)]
        elif isinstance(self.root, Hit):
            children = []
            self.hits = [self.root]
        else:
            children = []
            self.hits = []
        # Convenience groups are exactly the direct ResultGroup children of
        # the canonical root. Do not infer wrapper semantics from response
        # shape; request-derived conveniences (such as facets) are projected
        # separately by the builder that knows the request.
        self.groups = [child for child in children if isinstance(child, ResultGroup)]
        self.total_hits = result.totalHits.value
        self.total_hits_relation = result.totalHits.relation
        coverage = getattr(result, "coverage", None)
        self.coverage = Coverage(coverage) if coverage is not None else None
        timing = getattr(result, "timing", None)
        self.timing = SearchTiming(timing) if timing is not None else None
        self.errors = [ResultIssue(error) for error in (getattr(result, "errors", None) or [])]
        self.issues = self.errors
        self.trace = _python_value(getattr(result, "trace", None))
        self.metadata = _value_entries(getattr(result, "metadata", None))
        self.aggregations = dict(self.root) if isinstance(self.root, ResultGroup) else {}
        self.continuations = (
            list(getattr(self.root, "continuations", []) or [])
            if self.root is not None
            else []
        )
        self.facets = {}

    def _project_facet(self, expected_label, result_name, expected_field=None):
        expected_field = expected_label if expected_field is None else expected_field
        candidates = [
            group
            for group in self.groups
            if (group.id == expected_field or group.id is None)
            and group.label == expected_label
            and group.value is None
            and not group._field_entries
            and not group.continuations
            and group.source is None
            and not group.types
            and group.relevance is None
            and group.limits is None
            and not group.metadata
            and len(group.groups) == len(group.children)
        ]
        if len(candidates) != 1:
            return

        buckets = []
        for child in candidates[0].groups:
            optimized_child = child.id is None and child.label is None
            generic_child = child.id == "grouping-0" and child.label == expected_label
            if not (optimized_child or generic_child):
                return
            if child.children or child.continuations:
                return
            if child.source is not None or child.types or child.relevance is not None:
                return
            if child.limits is not None or child.metadata:
                return
            fields = child._field_entries
            if len(fields) != 1:
                return
            count = fields[0]
            accepted_count_kinds = (
                (ValueKind.Int64Value, ValueKind.UInt64Value)
                if generic_child
                else (ValueKind.UInt64Value,)
            )
            if (
                count.key != "count"
                or count.value is None
                or count.value.kindType not in accepted_count_kinds
            ):
                return
            count_value = _python_value(count.value)
            if generic_child and count_value < 0:
                return
            buckets.append({"value": child.value, "count": count_value})

        self.facets[result_name] = buckets


__all__ = [
    "Boolean", "Coverage", "CoverageDegraded", "Float", "Group", "Hit",
    "Integer", "Keyword", "PatchBuilder", "RankProfile", "ResultContinuation",
    "ResultGroup", "ResultIssue", "ResultLimits", "SearchBuilder", "SearchResults",
    "SearchTiming", "Tensor", "TensorField", "Text", "UpdateBuilder", "YqlBuilder",
    "agg", "asc", "desc", "g", "gf", "q",
]
