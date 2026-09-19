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
from SearchKernel.V2.QueryContains import QueryContainsT
from SearchKernel.V2.QueryExpr import QueryExprT
from SearchKernel.V2.QueryExprKind import QueryExprKind
from SearchKernel.V2.QueryFieldRef import QueryFieldRefT
from SearchKernel.V2.QueryInput import QueryInputT
from SearchKernel.V2.QueryInputKind import QueryInputKind
from SearchKernel.V2.LiteralQueryInput import LiteralQueryInputT
from SearchKernel.V2.ExpressionQueryInput import ExpressionQueryInputT
from SearchKernel.V2.QuerySource import QuerySourceT
from SearchKernel.V2.QuerySourceKind import QuerySourceKind
from SearchKernel.V2.SearchField import SearchFieldT
from SearchKernel.V2.SearchFieldSelection import SearchFieldSelectionT
from SearchKernel.V2.SearchKernelTextQuery import SearchKernelTextQueryT
from SearchKernel.V2.MatchAllQuery import MatchAllQueryT
from SearchKernel.V2.YqlQuery import YqlQueryT
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
from SearchKernel.V2.SetField import SetFieldT
from SearchKernel.V2.UnsetField import UnsetFieldT
from SearchKernel.V2.PatchOperation import PatchOperationT
from SearchKernel.V2.PatchOperationKind import PatchOperationKind
from SearchKernel.V2.PatchRequest import PatchRequestT
from SearchKernel.V2.ResultNodeKind import ResultNodeKind

from .client import _python_value, value


class _FieldSpec:
    kind = "string"
    matching = "unspecified"

    def __init__(self, *, stored=None, searchable=None, filterable=None, sortable=None, facetable=None):
        self.stored, self.searchable = stored, searchable
        self.filterable, self.sortable, self.facetable = filterable, sortable, facetable

    def definition(self, name: str) -> FieldDefinitionT:
        primitive = {
            "string": 1, "text": 1, "keyword": 1, "int64": 5, "float64": 7, "bool": 2
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
                    mode={"unspecified": 0, "text": 1, "exact": 3}.get(self.matching, 0)
                )
                if primitive == 1
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

    def __add__(self, other): return _binary(1, self, other)
    def __sub__(self, other): return _binary(2, self, other)
    def __mul__(self, other): return _binary(3, self, other)
    def __truediv__(self, other): return _binary(4, self, other)


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

    def and_(self, *clauses): return GroupingFilterExprT(exprType=GroupingFilterExprKind.GroupingFilterAnd, expr=GroupingFilterAndT(clauses=list(clauses)))
    def or_(self, *clauses): return GroupingFilterExprT(exprType=GroupingFilterExprKind.GroupingFilterOr, expr=GroupingFilterOrT(clauses=list(clauses)))
    def not_(self, clause): return GroupingFilterExprT(exprType=GroupingFilterExprKind.GroupingFilterNot, expr=GroupingFilterNotT(clause=clause))


gf = _GroupingFilterNamespace()


class _QueryNamespace:
    def contains(self, field: str, text: str) -> QuerySourceT:
        term = QueryExprT(exprType=QueryExprKind.TermQuery, expr=__import__("SearchKernel.V2.TermQuery", fromlist=["TermQueryT"]).TermQueryT(value=value(text)))
        contains = QueryExprT(exprType=QueryExprKind.QueryContains, expr=QueryContainsT(field=QueryFieldRefT(path=field), item=term))
        return QuerySourceT(sourceType=QuerySourceKind.TypedQuery, source=__import__("SearchKernel.V2.TypedQuery", fromlist=["TypedQueryT"]).TypedQueryT(query=contains))


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

    def _operations(self):
        operations = [GroupingOperationT(operationType=GroupingOperationKind.GroupingGroupOperation, operation=GroupingGroupOperationT(expression=self.expression.model))]
        operations += [GroupingOperationT(operationType=GroupingOperationKind.GroupingAliasOperation, operation=GroupingAliasOperationT(name=n, expression=e.model)) for n, e in self._aliases]
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
            operations.append(GroupingOperationT(operationType=GroupingOperationKind.GroupingEachOperation, operation=GroupingEachOperationT(operations=subgroup._operations())))
        return operations

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


class SearchBuilder:
    def __init__(self, index, query):
        self.index, self.query = index, query
        self._filters, self._sort, self._fields, self._groups = [], [], None, []
        self._aggregates, self._facets = [], []
        self._ranking, self._rank_properties, self._inputs, self._properties = None, {}, {}, {}
        self._limit = self._offset = None
        self._paged = False

    def filter(self, source, bindings=None): self._filters.append((source, dict(bindings or {}))); return self
    def sort(self, field, direction="asc"):
        if direction not in {"asc", "desc"}: raise ValueError("sort direction must be 'asc' or 'desc'")
        self._sort.append(SortExprT(target=SortTargetT(targetType=SortTargetKind.FieldSortTarget, target=FieldSortTargetT(field=field)), direction=SortDirection.Asc if direction == "asc" else SortDirection.Desc)); return self
    def fields(self, *fields): self._fields = list(fields); return self
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
                bindings=[ValueEntryT(key=name, value=value(item)) for name, item in bindings.items()],
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
            groups.append(Group.by(g.value(0)).output(*self._aggregates).to_request())
        query = self.query
        if query is None: query = QuerySourceT(sourceType=QuerySourceKind.MatchAllQuery, source=MatchAllQueryT())
        elif isinstance(query, str): query = QuerySourceT(sourceType=QuerySourceKind.SearchKernelTextQuery, source=SearchKernelTextQueryT(text=query))
        inputs = [QueryInputT(name=name, inputType=QueryInputKind.LiteralQueryInput, input=LiteralQueryInputT(value=value(item))) for name, item in self._inputs.items()]
        ranking = RankingRequestT(profile=self._ranking, properties=[StringEntryT(key=k, value=v) for k, v in self._rank_properties.items()]) if self._ranking is not None else None
        presentation = PresentationRequestT(fields=FieldProjectionT(fields=self._fields)) if self._fields is not None else None
        request = SearchRequestT(query=query, filters=filters, sort=self._sort, grouping=groups, ranking=ranking, inputs=inputs, limit=OptionalUInt32T(value=self._limit) if self._limit is not None else None, offset=OptionalUInt32T(value=self._offset) if self._offset is not None else None, presentation=presentation, properties=[ValueEntryT(key=k, value=v) for k, v in self._properties.items()])
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
    def __init__(self, index, identifier): self.index, self.identifier = index, identifier
    def increment(self, field, item): self.field, self.item = field, item; return self
    def execute(self): raise NotImplementedError("UpdateRequest is not part of the current canonical V2 operation envelope")


class Hit(dict):
    def __init__(self, node):
        super().__init__({entry.key: _python_value(entry.value) for entry in (node.fields or [])})
        self.id, self.fields = node.id or "", self
        self.relevance = node.relevance.value if node.relevance is not None else None


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
        self.groups = [child for child in children if isinstance(child, ResultGroup)]
        self.total_hits = result.totalHits.value if result.totalHits is not None else 0
        self.total_hits_relation = result.totalHits.relation if result.totalHits is not None else "unknown"
        self.aggregations = dict(self.root) if isinstance(self.root, ResultGroup) else {}
        self.facets = {}
        for group in self.groups:
            for child in group.groups or [group]:
                name = child.label or group.label or ""
                self.facets.setdefault(name, []).append({"value": child.value, "count": child.get(name, child.get("count", 0))})


__all__ = ["Boolean", "Float", "Group", "Hit", "Integer", "Keyword", "PatchBuilder", "ResultGroup", "SearchBuilder", "SearchResults", "Text", "UpdateBuilder", "YqlBuilder", "agg", "asc", "desc", "g", "gf", "q"]
