"""Canonical V2 FlatBuffer encoding and decoding.

The generated modules in ``_wire_generated`` own the binary layout.  This
module only maps the public generated domain models to those generated tables
and never writes FlatBuffer offsets or vtables by hand.
"""

from __future__ import annotations

import sys
from pathlib import Path
from typing import Any

import flatbuffers

_GENERATED_ROOT = str(Path(__file__).with_name("_wire_generated"))
if _GENERATED_ROOT not in sys.path:
    sys.path.insert(0, _GENERATED_ROOT)

from SearchKernel.Protocol.V2 import RequestEnvelope as RequestEnvelope  # noqa: E402
from SearchKernel.Protocol.V2 import RequestPayload as RequestPayload  # noqa: E402
from SearchKernel.Protocol.V2 import ResponseEnvelope as ResponseEnvelope  # noqa: E402
from SearchKernel.Protocol.V2 import ResponsePayload as ResponsePayload  # noqa: E402
from SearchKernel.V2 import AnalyzeFieldTarget as AnalyzeFieldTarget  # noqa: E402
from SearchKernel.V2 import AnalyzeRequest as AnalyzeRequest  # noqa: E402
from SearchKernel.V2 import AnalyzeResult as AnalyzeResult  # noqa: E402
from SearchKernel.V2 import AnalyzeTarget as AnalyzeTarget  # noqa: E402
from SearchKernel.V2 import AnalyzedToken as AnalyzedToken  # noqa: E402
from SearchKernel.V2 import BoolValue as BoolValue  # noqa: E402
from SearchKernel.V2 import BytesValue as BytesValue  # noqa: E402
from SearchKernel.V2 import DataType as DataType  # noqa: E402
from SearchKernel.V2 import DataTypeKind as DataTypeKind  # noqa: E402
from SearchKernel.V2 import DeleteRequest as DeleteRequest  # noqa: E402
from SearchKernel.V2 import DeleteResult as DeleteResult  # noqa: E402
from SearchKernel.V2 import DescribeRequest as DescribeRequest  # noqa: E402
from SearchKernel.V2 import DescribeResult as DescribeResult  # noqa: E402
from SearchKernel.V2 import Document as Document  # noqa: E402
from SearchKernel.V2 import Error as Error  # noqa: E402
from SearchKernel.V2 import ErrorCode as ErrorCode  # noqa: E402
from SearchKernel.V2 import ErrorDetail as ErrorDetail  # noqa: E402
from SearchKernel.V2 import FieldDefinition as FieldDefinition  # noqa: E402
from SearchKernel.V2 import FieldProjection as FieldProjection  # noqa: E402
from SearchKernel.V2 import FieldSortTarget as FieldSortTarget  # noqa: E402
from SearchKernel.V2 import FilterAnd as FilterAnd  # noqa: E402
from SearchKernel.V2 import FilterComparison as FilterComparison  # noqa: E402
from SearchKernel.V2 import FilterExists as FilterExists  # noqa: E402
from SearchKernel.V2 import FilterExpr as FilterExpr  # noqa: E402
from SearchKernel.V2 import FilterExprKind as FilterExprKind  # noqa: E402
from SearchKernel.V2 import FilterIn as FilterIn  # noqa: E402
from SearchKernel.V2 import FilterRequest as FilterRequest  # noqa: E402
from SearchKernel.V2 import FilterNot as FilterNot  # noqa: E402
from SearchKernel.V2 import FilterOr as FilterOr  # noqa: E402
from SearchKernel.V2 import Float64Value as Float64Value  # noqa: E402
from SearchKernel.V2 import GetRequest as GetRequest  # noqa: E402
from SearchKernel.V2 import GetResult as GetResult  # noqa: E402
from SearchKernel.V2 import GroupingRequest as GroupingRequest  # noqa: E402
from SearchKernel.V2 import GroupingAllOperation as GroupingAllOperation  # noqa: E402
from SearchKernel.V2 import GroupingAggregationExpression as GroupingAggregationExpression  # noqa: E402
from SearchKernel.V2 import GroupingAttributeExpression as GroupingAttributeExpression  # noqa: E402
from SearchKernel.V2 import GroupingEachOperation as GroupingEachOperation  # noqa: E402
from SearchKernel.V2 import GroupingExpression as GroupingExpression  # noqa: E402
from SearchKernel.V2 import GroupingGroupOperation as GroupingGroupOperation  # noqa: E402
from SearchKernel.V2 import GroupingMaxKind as GroupingMaxKind  # noqa: E402
from SearchKernel.V2 import GroupingMaxOperation as GroupingMaxOperation  # noqa: E402
from SearchKernel.V2 import FiniteGroupingMax as FiniteGroupingMax  # noqa: E402
from SearchKernel.V2 import GroupingOrderExpr as GroupingOrderExpr  # noqa: E402
from SearchKernel.V2 import GroupingOrderOperation as GroupingOrderOperation  # noqa: E402
from SearchKernel.V2 import GroupingOperation as GroupingOperation  # noqa: E402
from SearchKernel.V2 import GroupingOperationKind as GroupingOperationKind  # noqa: E402
from SearchKernel.V2 import GroupingOutput as GroupingOutput  # noqa: E402
from SearchKernel.V2 import GroupingOutputOperation as GroupingOutputOperation  # noqa: E402
from SearchKernel.V2 import GroupingSourceKind as GroupingSourceKind  # noqa: E402
from SearchKernel.V2 import GroupingUnaryExpression as GroupingUnaryExpression  # noqa: E402
from SearchKernel.V2 import GroupingUnaryOperator as GroupingUnaryOperator  # noqa: E402
from SearchKernel.V2 import StructuredGroupingSource as StructuredGroupingSource  # noqa: E402
from SearchKernel.V2 import Hit as Hit  # noqa: E402
from SearchKernel.V2 import IndexDefinition as IndexDefinition  # noqa: E402
from SearchKernel.V2 import InsertRequest as InsertRequest  # noqa: E402
from SearchKernel.V2 import InsertResult as InsertResult  # noqa: E402
from SearchKernel.V2 import InspectQueryRequest as InspectQueryRequest  # noqa: E402
from SearchKernel.V2 import InspectQueryResult as InspectQueryResult  # noqa: E402
from SearchKernel.V2 import Int64Value as Int64Value  # noqa: E402
from SearchKernel.V2 import MatchingConfig as MatchingConfig  # noqa: E402
from SearchKernel.V2 import MatchingMode as MatchingMode  # noqa: E402
from SearchKernel.V2 import OptionalBool as OptionalBool  # noqa: E402
from SearchKernel.V2 import OptionalUInt32 as OptionalUInt32  # noqa: E402
from SearchKernel.V2 import PrimitiveDataType as PrimitiveDataType  # noqa: E402
from SearchKernel.V2 import PrimitiveDataTypeKind as PrimitiveDataTypeKind  # noqa: E402
from SearchKernel.V2 import QueryFieldRef as QueryFieldRef  # noqa: E402
from SearchKernel.V2 import QuerySource as QuerySource  # noqa: E402
from SearchKernel.V2 import QuerySourceKind as QuerySourceKind  # noqa: E402
from SearchKernel.V2 import ResolvedSearchRequest as ResolvedSearchRequest  # noqa: E402
from SearchKernel.V2 import ResolvedSortTargetKind as ResolvedSortTargetKind  # noqa: E402
from SearchKernel.V2 import ResultGroup as ResultGroup  # noqa: E402
from SearchKernel.V2 import ResultNode as ResultNode  # noqa: E402
from SearchKernel.V2 import ResultNodeKind as ResultNodeKind  # noqa: E402
from SearchKernel.V2 import SearchField as SearchField  # noqa: E402
from SearchKernel.V2 import SearchFieldSelection as SearchFieldSelection  # noqa: E402
from SearchKernel.V2 import SearchKernelTextQuery as SearchKernelTextQuery  # noqa: E402
from SearchKernel.V2 import SearchRequest as SearchRequest  # noqa: E402
from SearchKernel.V2 import SearchResult as SearchResult  # noqa: E402
from SearchKernel.V2 import SortDirection as SortDirection  # noqa: E402
from SearchKernel.V2 import SortExpr as SortExpr  # noqa: E402
from SearchKernel.V2 import SortTarget as SortTarget  # noqa: E402
from SearchKernel.V2 import SortTargetKind as SortTargetKind  # noqa: E402
from SearchKernel.V2 import StringValue as StringValue  # noqa: E402
from SearchKernel.V2 import TotalHits as TotalHits  # noqa: E402
from SearchKernel.V2 import TotalHitsRelation as TotalHitsRelation  # noqa: E402
from SearchKernel.V2 import UInt64Value as UInt64Value  # noqa: E402
from SearchKernel.V2 import UpsertRequest as UpsertRequest  # noqa: E402
from SearchKernel.V2 import UpsertResult as UpsertResult  # noqa: E402
from SearchKernel.V2 import Value as Value  # noqa: E402
from SearchKernel.V2 import ValueEntry as ValueEntry  # noqa: E402
from SearchKernel.V2 import ValueKind as ValueKind  # noqa: E402
from SearchKernel.V2 import ValueList as ValueList  # noqa: E402
from SearchKernel.V2 import ValueObject as ValueObject  # noqa: E402

from .generated import models

RequestPayload = RequestPayload.RequestPayload
ResponsePayload = ResponsePayload.ResponsePayload
DataTypeKind = DataTypeKind.DataTypeKind
AnalyzeTarget = AnalyzeTarget.AnalyzeTarget
FilterExprKind = FilterExprKind.FilterExprKind
GroupingOperationKind = GroupingOperationKind.GroupingOperationKind
GroupingMaxKind = GroupingMaxKind.GroupingMaxKind
GroupingUnaryOperator = GroupingUnaryOperator.GroupingUnaryOperator
GroupingSourceKind = GroupingSourceKind.GroupingSourceKind
PrimitiveDataTypeKind = PrimitiveDataTypeKind.PrimitiveDataTypeKind
QuerySourceKind = QuerySourceKind.QuerySourceKind
ResultNodeKind = ResultNodeKind.ResultNodeKind
SortTargetKind = SortTargetKind.SortTargetKind
TotalHitsRelation = TotalHitsRelation.TotalHitsRelation
ValueKind = ValueKind.ValueKind
MatchingMode = MatchingMode.MatchingMode
SortDirection = SortDirection.SortDirection
ResolvedSortTargetKind = ResolvedSortTargetKind.ResolvedSortTargetKind
PROTOCOL_VERSION = 2


def _string(value: Any) -> str | None:
    if value is None:
        return None
    return value.decode("utf-8") if isinstance(value, (bytes, bytearray)) else str(value)


def _reject_populated(path: str, **fields: Any) -> None:
    """Reject fields that this partial encoder cannot represent."""
    for name, value in fields.items():
        if value is None:
            continue
        if isinstance(value, (str, bytes, bytearray, dict, list, tuple, set)) and not value:
            continue
        raise ValueError(f"unsupported populated field {path}.{name}")


def _vector(builder: flatbuffers.Builder, offsets: list[int]) -> int | None:
    if not offsets:
        return None
    builder.StartVector(4, len(offsets), 4)
    for offset in reversed(offsets):
        builder.PrependUOffsetTRelative(offset)
    return builder.EndVector()


def _strings(builder: flatbuffers.Builder, values: list[str]) -> int | None:
    return _vector(builder, [builder.CreateString(value) for value in values])


def _entries(builder: flatbuffers.Builder, values: dict[str, Any]) -> int | None:
    offsets = []
    for key in sorted(values):
        key_offset = builder.CreateString(key)
        value_offset = _value(builder, values[key])
        ValueEntry.Start(builder)
        ValueEntry.AddKey(builder, key_offset)
        ValueEntry.AddValue(builder, value_offset)
        offsets.append(ValueEntry.End(builder))
    return _vector(builder, offsets)


def _value(builder: flatbuffers.Builder, value: models.Value) -> int:
    if not isinstance(value, models.Value):
        raise TypeError("value must be a generated Value")
    _reject_populated("value", payload=value.payload, type_spec=value.type_spec)
    if value.kind == "null":
        from SearchKernel.V2 import NullValueValue

        NullValueValue.Start(builder)
        payload = NullValueValue.End(builder)
        tag = ValueKind.NullValueValue
    elif value.kind == "bool":
        BoolValue.Start(builder)
        BoolValue.AddValue(builder, bool(value.value))
        payload = BoolValue.End(builder)
        tag = ValueKind.BoolValue
    elif value.kind == "int64":
        Int64Value.Start(builder)
        Int64Value.AddValue(builder, int(value.value))
        payload = Int64Value.End(builder)
        tag = ValueKind.Int64Value
    elif value.kind == "uint64":
        UInt64Value.Start(builder)
        UInt64Value.AddValue(builder, int(value.value))
        payload = UInt64Value.End(builder)
        tag = ValueKind.UInt64Value
    elif value.kind == "float64":
        Float64Value.Start(builder)
        Float64Value.AddValue(builder, float(value.value))
        payload = Float64Value.End(builder)
        tag = ValueKind.Float64Value
    elif value.kind == "string":
        text = builder.CreateString(value.value)
        StringValue.Start(builder)
        StringValue.AddValue(builder, text)
        payload = StringValue.End(builder)
        tag = ValueKind.StringValue
    elif value.kind == "bytes":
        BytesValue.Start(builder)
        BytesValue.AddValue(builder, builder.CreateByteVector(bytes(value.value)))
        payload = BytesValue.End(builder)
        tag = ValueKind.BytesValue
    elif value.kind == "list":
        payload_offsets = [_value(builder, item) for item in value.value]
        vector = _vector(builder, payload_offsets)
        ValueList.Start(builder)
        if vector is not None:
            ValueList.AddValues(builder, vector)
        payload = ValueList.End(builder)
        tag = ValueKind.ValueList
    elif value.kind == "object":
        entries = _entries(builder, value.value)
        ValueObject.Start(builder)
        if entries is not None:
            ValueObject.AddFields(builder, entries)
        payload = ValueObject.End(builder)
        tag = ValueKind.ValueObject
    else:
        raise ValueError(f"unknown Value kind {value.kind!r}")
    Value.Start(builder)
    Value.AddKindType(builder, tag)
    Value.AddKind(builder, payload)
    return Value.End(builder)


def _document(builder: flatbuffers.Builder, value: models.Document | None) -> int | None:
    if value is None:
        return None
    fields = _entries(builder, value.fields)
    identifier = builder.CreateString(value.id) if value.id is not None else None
    Document.Start(builder)
    if identifier is not None:
        Document.AddId(builder, identifier)
    if fields is not None:
        Document.AddFields(builder, fields)
    return Document.End(builder)


def _field_ref(builder: flatbuffers.Builder, value: Any) -> int:
    if isinstance(value, models.QueryFieldRef):
        if value.path is None:
            raise ValueError("query field reference path is required")
        path = value.path
        element_index = value.element_index
    else:
        path = str(value)
        element_index = None
    path_offset = builder.CreateString(path)
    QueryFieldRef.Start(builder)
    QueryFieldRef.AddPath(builder, path_offset)
    if element_index is not None:
        QueryFieldRef.AddElementIndex(
            builder, OptionalUInt32.CreateOptionalUint32(builder, element_index)
        )
    return QueryFieldRef.End(builder)


def _filter(builder: flatbuffers.Builder, value: models.FilterExpr) -> int:
    kind = value.kind
    if kind in {"and", "or"}:
        _reject_populated(
            f"filter.{kind}",
            clause=value.clause,
            field=value.field,
            include_lower=value.include_lower,
            lower=value.lower,
            op=value.op,
            upper=value.upper,
            value=value.value,
            values=value.values,
        )
        module = FilterAnd if kind == "and" else FilterOr
        offsets = [_filter(builder, item) for item in value.clauses]
        vector = _vector(builder, offsets)
        module.Start(builder)
        module.AddClauses(builder, vector)
        payload = module.End(builder)
        tag = FilterExprKind.FilterAnd if kind == "and" else FilterExprKind.FilterOr
    elif kind == "not":
        _reject_populated(
            "filter.not",
            clauses=value.clauses,
            field=value.field,
            include_lower=value.include_lower,
            lower=value.lower,
            op=value.op,
            upper=value.upper,
            value=value.value,
            values=value.values,
        )
        clause = _filter(builder, value.clause)
        FilterNot.Start(builder)
        FilterNot.AddClause(builder, clause)
        payload, tag = FilterNot.End(builder), FilterExprKind.FilterNot
    elif kind == "exists":
        _reject_populated(
            "filter.exists",
            clause=value.clause,
            clauses=value.clauses,
            include_lower=value.include_lower,
            lower=value.lower,
            op=value.op,
            upper=value.upper,
            value=value.value,
            values=value.values,
        )
        field = _field_ref(builder, value.field)
        FilterExists.Start(builder)
        FilterExists.AddField(builder, field)
        payload, tag = FilterExists.End(builder), FilterExprKind.FilterExists
    elif kind == "comparison":
        _reject_populated(
            "filter.comparison",
            clause=value.clause,
            clauses=value.clauses,
            include_lower=value.include_lower,
            lower=value.lower,
            upper=value.upper,
            values=value.values,
        )
        field = _field_ref(builder, value.field)
        operand = _value(builder, value.value)
        FilterComparison.Start(builder)
        FilterComparison.AddField(builder, field)
        try:
            op = {"unspecified": 0, "eq": 1, "ne": 2, "lt": 3, "lte": 4, "gt": 5, "gte": 6}[value.op]
        except KeyError as error:
            raise ValueError(f"unsupported filter comparison operator {value.op!r}") from error
        FilterComparison.AddOp(builder, op)
        FilterComparison.AddValue(builder, operand)
        payload, tag = FilterComparison.End(builder), FilterExprKind.FilterComparison
    else:
        raise ValueError(f"unsupported filter kind {kind!r}")
    FilterExpr.Start(builder)
    FilterExpr.AddExprType(builder, tag)
    FilterExpr.AddExpr(builder, payload)
    return FilterExpr.End(builder)


def _query(builder: flatbuffers.Builder, value: models.QuerySource | None) -> int | None:
    if value is None:
        return None
    if value.kind in {"text", "search_kernel_text"}:
        _reject_populated("query", parameters=value.parameters, query=value.query, source=value.source)
        text = builder.CreateString(value.text or "")
        SearchKernelTextQuery.Start(builder)
        SearchKernelTextQuery.AddText(builder, text)
        source, tag = SearchKernelTextQuery.End(builder), QuerySourceKind.SearchKernelTextQuery
    elif value.kind in {"match_all", "all"}:
        _reject_populated(
            "query",
            parameters=value.parameters,
            query=value.query,
            source=value.source,
            text=value.text,
        )
        from SearchKernel.V2 import MatchAllQuery

        MatchAllQuery.Start(builder)
        source, tag = MatchAllQuery.End(builder), QuerySourceKind.MatchAllQuery
    else:
        raise ValueError(f"unsupported query source {value.kind!r}")
    QuerySource.Start(builder)
    QuerySource.AddSourceType(builder, tag)
    QuerySource.AddSource(builder, source)
    return QuerySource.End(builder)


def _search_field_selection(builder: flatbuffers.Builder, value: models.SearchFieldSelection | None) -> int | None:
    if value is None:
        return None
    offsets = []
    for item in value.fields:
        field = builder.CreateString(item.field)
        SearchField.Start(builder)
        SearchField.AddField(builder, field)
        if item.weight is not None:
            from SearchKernel.V2 import OptionalDouble

            SearchField.AddWeight(builder, OptionalDouble.CreateOptionalDouble(builder, item.weight))
        offsets.append(SearchField.End(builder))
    vector = _vector(builder, offsets)
    SearchFieldSelection.Start(builder)
    if vector is not None:
        SearchFieldSelection.AddFields(builder, vector)
    return SearchFieldSelection.End(builder)


def _sort(builder: flatbuffers.Builder, value: models.SortExpr) -> int:
    target = value.target
    if not isinstance(target, models.SortTarget) or target.kind != "field":
        raise ValueError("unsupported populated field search.sort.target")
    _reject_populated("search.sort.target", expression=target.expression, feature=target.feature)
    _reject_populated("search.sort", missing=value.missing, annotations=value.annotations)
    if not target.field:
        raise ValueError("search.sort.target.field is required")
    field_name = builder.CreateString(target.field)
    FieldSortTarget.Start(builder)
    FieldSortTarget.AddField(builder, field_name)
    field_target = FieldSortTarget.End(builder)
    SortTarget.Start(builder)
    SortTarget.AddTargetType(builder, SortTargetKind.FieldSortTarget)
    SortTarget.AddTarget(builder, field_target)
    sort_target = SortTarget.End(builder)
    SortExpr.Start(builder)
    SortExpr.AddTarget(builder, sort_target)
    try:
        direction = {"unspecified": 0, "asc": 1, "desc": 2}[value.direction]
    except KeyError as error:
        raise ValueError(f"unsupported sort direction {value.direction!r}") from error
    SortExpr.AddDirection(builder, direction)
    return SortExpr.End(builder)


def _grouping_expression(builder: flatbuffers.Builder, value: models.GroupingExpression) -> int:
    if value.kind == "attribute":
        _reject_populated(
            "grouping.expression.attribute",
            aggregation=value.aggregation,
            arguments=value.arguments,
            base=value.base,
            expression=value.expression,
            include_lower=value.include_lower,
            key=value.key,
            left=value.left,
            lower=value.lower,
            member=value.member,
            name=value.name,
            op=value.op,
            right=value.right,
            upper=value.upper,
            value=value.value,
        )
        field = builder.CreateString(value.field or "")
        GroupingAttributeExpression.Start(builder)
        GroupingAttributeExpression.AddField(builder, field)
        expression, tag = GroupingAttributeExpression.End(builder), 1
    elif value.kind == "aggregation":
        from SearchKernel.V2 import AggregationKind, CountAggregation

        _reject_populated(
            "grouping.expression.aggregation",
            arguments=value.arguments,
            base=value.base,
            expression=value.expression,
            field=value.field,
            include_lower=value.include_lower,
            key=value.key,
            left=value.left,
            lower=value.lower,
            member=value.member,
            name=value.name,
            op=value.op,
            right=value.right,
            upper=value.upper,
            value=value.value,
        )
        if value.aggregation is not None and (
            value.aggregation.kind != "count" or value.aggregation.value is not None
        ):
            raise ValueError("unsupported populated field grouping.expression.aggregation")
        CountAggregation.Start(builder)
        aggregation = CountAggregation.End(builder)
        GroupingAggregationExpression.Start(builder)
        GroupingAggregationExpression.AddAggregationType(builder, AggregationKind.AggregationKind.CountAggregation)
        GroupingAggregationExpression.AddAggregation(builder, aggregation)
        aggregation_expression = GroupingAggregationExpression.End(builder)
        expression, tag = aggregation_expression, 10
    elif value.kind == "negative":
        _reject_populated(
            "grouping.expression.negative",
            aggregation=value.aggregation,
            arguments=value.arguments,
            base=value.base,
            field=value.field,
            include_lower=value.include_lower,
            key=value.key,
            left=value.left,
            lower=value.lower,
            member=value.member,
            name=value.name,
            op=value.op,
            right=value.right,
            upper=value.upper,
            value=value.value,
        )
        nested = _grouping_expression(builder, value.expression)
        GroupingUnaryExpression.Start(builder)
        GroupingUnaryExpression.AddOp(builder, GroupingUnaryOperator.Negative)
        GroupingUnaryExpression.AddExpression(builder, nested)
        expression, tag = GroupingUnaryExpression.End(builder), 4
    else:
        raise ValueError(f"unsupported grouping expression kind {value.kind!r}")
    GroupingExpression.Start(builder)
    GroupingExpression.AddExprType(builder, tag)
    GroupingExpression.AddExpr(builder, expression)
    return GroupingExpression.End(builder)


def _grouping_operation(builder: flatbuffers.Builder, value: models.GroupingOperation) -> int:
    if value.kind == "group":
        _reject_populated(
            "grouping.operation.group",
            expressions=value.expressions,
            filter=value.filter,
            label=value.label,
            name=value.name,
            operations=value.operations,
            outputs=value.outputs,
            value=value.value,
        )
        expression = _grouping_expression(builder, value.expression)
        GroupingGroupOperation.Start(builder)
        GroupingGroupOperation.AddExpression(builder, expression)
        operation, tag = GroupingGroupOperation.End(builder), GroupingOperationKind.GroupingGroupOperation
    elif value.kind == "output":
        _reject_populated(
            "grouping.operation.output",
            expression=value.expression,
            expressions=value.expressions,
            filter=value.filter,
            label=value.label,
            name=value.name,
            operations=value.operations,
            value=value.value,
        )
        outputs = []
        for output in value.outputs:
            label = builder.CreateString(output.label) if output.label is not None else None
            expression = _grouping_expression(builder, output.expression)
            GroupingOutput.Start(builder)
            if label is not None:
                GroupingOutput.AddLabel(builder, label)
            GroupingOutput.AddExpression(builder, expression)
            outputs.append(GroupingOutput.End(builder))
        output_vector = _vector(builder, outputs)
        GroupingOutputOperation.Start(builder)
        GroupingOutputOperation.AddOutputs(builder, output_vector)
        operation, tag = GroupingOutputOperation.End(builder), GroupingOperationKind.GroupingOutputOperation
    elif value.kind == "each":
        _reject_populated(
            "grouping.operation.each",
            expression=value.expression,
            expressions=value.expressions,
            filter=value.filter,
            name=value.name,
            outputs=value.outputs,
            value=value.value,
        )
        operations = _vector(builder, [_grouping_operation(builder, item) for item in value.operations])
        label = builder.CreateString(value.label) if value.label is not None else None
        GroupingEachOperation.Start(builder)
        GroupingEachOperation.AddOperations(builder, operations)
        if label is not None:
            GroupingEachOperation.AddLabel(builder, label)
        operation, tag = GroupingEachOperation.End(builder), GroupingOperationKind.GroupingEachOperation
    elif value.kind == "max":
        _reject_populated(
            "grouping.operation.max",
            expression=value.expression,
            expressions=value.expressions,
            filter=value.filter,
            label=value.label,
            name=value.name,
            operations=value.operations,
            outputs=value.outputs,
        )
        FiniteGroupingMax.Start(builder)
        FiniteGroupingMax.AddValue(builder, int(value.value if value.value is not None else 10))
        finite = FiniteGroupingMax.End(builder)
        GroupingMaxOperation.Start(builder)
        GroupingMaxOperation.AddValueType(builder, GroupingMaxKind.FiniteGroupingMax)
        GroupingMaxOperation.AddValue(builder, finite)
        operation, tag = GroupingMaxOperation.End(builder), GroupingOperationKind.GroupingMaxOperation
    elif value.kind == "order":
        _reject_populated(
            "grouping.operation.order",
            expression=value.expression,
            filter=value.filter,
            label=value.label,
            name=value.name,
            operations=value.operations,
            outputs=value.outputs,
            value=value.value,
        )
        expressions = []
        for item in value.expressions:
            expression = _grouping_expression(builder, item.expression)
            GroupingOrderExpr.Start(builder)
            GroupingOrderExpr.AddExpression(builder, expression)
            expressions.append(GroupingOrderExpr.End(builder))
        expression_vector = _vector(builder, expressions)
        GroupingOrderOperation.Start(builder)
        GroupingOrderOperation.AddExpressions(builder, expression_vector)
        operation, tag = GroupingOrderOperation.End(builder), GroupingOperationKind.GroupingOrderOperation
    else:
        raise ValueError(f"unsupported grouping operation kind {value.kind!r}")
    GroupingOperation.Start(builder)
    GroupingOperation.AddOperationType(builder, tag)
    GroupingOperation.AddOperation(builder, operation)
    return GroupingOperation.End(builder)


def _grouping(builder: flatbuffers.Builder, value: models.GroupingRequest) -> int:
    if value.source is None or value.source.kind != "structured":
        raise ValueError("only structured grouping sources are supported")
    root = value.source.value
    if not isinstance(root, models.GroupingAllOperation):
        raise TypeError("structured grouping source must contain GroupingAllOperation")
    operations = _vector(builder, [_grouping_operation(builder, item) for item in root.operations])
    label = builder.CreateString(root.label) if root.label is not None else None
    GroupingAllOperation.Start(builder)
    GroupingAllOperation.AddOperations(builder, operations)
    if label is not None:
        GroupingAllOperation.AddLabel(builder, label)
    all_operation = GroupingAllOperation.End(builder)
    StructuredGroupingSource.Start(builder)
    StructuredGroupingSource.AddRoot(builder, all_operation)
    source = StructuredGroupingSource.End(builder)
    GroupingRequest.Start(builder)
    GroupingRequest.AddSourceType(builder, GroupingSourceKind.StructuredGroupingSource)
    GroupingRequest.AddSource(builder, source)
    if value.continuations:
        GroupingRequest.AddContinuations(builder, _strings(builder, value.continuations))
    return GroupingRequest.End(builder)


def _search(builder: flatbuffers.Builder, value: models.SearchRequest) -> int:
    _reject_populated(
        "search",
        ranking=value.ranking,
        inputs=value.inputs,
        query_model=value.query_model,
        presentation=value.presentation,
        properties=value.properties,
        query_profile=value.query_profile,
        search_chain=value.search_chain,
    )
    query = _query(builder, value.query)
    fields = _search_field_selection(builder, value.search_fields)
    filter_request = None
    if value.filters is not None:
        _reject_populated(
            "search.filters",
            sources=value.filters.sources,
            parameters=value.filters.parameters,
        )
    if value.filters is not None and value.filters.structured:
        filters = _vector(builder, [_filter(builder, item) for item in value.filters.structured])
        FilterRequest.Start(builder)
        FilterRequest.AddStructured(builder, filters)
        filter_request = FilterRequest.End(builder)
    sort = _vector(builder, [_sort(builder, item) for item in value.sort]) if value.sort else None
    grouping = _vector(builder, [_grouping(builder, item) for item in value.grouping]) if value.grouping else None
    SearchRequest.Start(builder)
    if query is not None:
        SearchRequest.AddQuery(builder, query)
    if fields is not None:
        SearchRequest.AddSearchFields(builder, fields)
    if filter_request is not None:
        SearchRequest.AddFilters(builder, filter_request)
    if sort is not None:
        SearchRequest.AddSort(builder, sort)
    if grouping is not None:
        SearchRequest.AddGrouping(builder, grouping)
    if value.limit is not None:
        from SearchKernel.V2 import OptionalUInt32

        SearchRequest.AddLimit(builder, OptionalUInt32.CreateOptionalUint32(builder, value.limit))
    if value.offset is not None:
        from SearchKernel.V2 import OptionalUInt32

        SearchRequest.AddOffset(builder, OptionalUInt32.CreateOptionalUint32(builder, value.offset))
    if value.timeout_ms is not None:
        from SearchKernel.V2 import OptionalUInt64

        SearchRequest.AddTimeoutMs(builder, OptionalUInt64.CreateOptionalUint64(builder, value.timeout_ms))
    return SearchRequest.End(builder)


def _index_definition(builder: flatbuffers.Builder, value: models.IndexDefinition) -> int:
    fields = []
    primitive = {
        "string": PrimitiveDataTypeKind.String,
        "text": PrimitiveDataTypeKind.String,
        "keyword": PrimitiveDataTypeKind.String,
        "int64": PrimitiveDataTypeKind.Int64,
        "float64": PrimitiveDataTypeKind.Float64,
        "bool": PrimitiveDataTypeKind.Bool,
    }
    for item in value.fields:
        data = item.data_type
        kind = (data.payload_kind if data is not None and data.kind == "primitive" else data.kind) if data is not None else "string"
        PrimitiveDataType.Start(builder)
        PrimitiveDataType.AddKind(builder, primitive[kind])
        primitive_offset = PrimitiveDataType.End(builder)
        DataType.Start(builder)
        DataType.AddKindType(builder, DataTypeKind.PrimitiveDataType)
        DataType.AddKind(builder, primitive_offset)
        data_offset = DataType.End(builder)
        name = builder.CreateString(item.name)
        matching = None
        if kind == "keyword":
            MatchingConfig.Start(builder)
            MatchingConfig.AddMode(builder, MatchingMode.Exact)
            matching = MatchingConfig.End(builder)
        FieldDefinition.Start(builder)
        FieldDefinition.AddName(builder, name)
        FieldDefinition.AddDataType(builder, data_offset)
        if matching is not None:
            FieldDefinition.AddMatching(builder, matching)
        for name, add in (("stored", FieldDefinition.AddStored), ("searchable", FieldDefinition.AddSearchable),
                          ("filterable", FieldDefinition.AddFilterable), ("sortable", FieldDefinition.AddSortable),
                          ("facetable", FieldDefinition.AddFacetable)):
            setting = getattr(item, name)
            if setting is not None:
                add(builder, OptionalBool.CreateOptionalBool(builder, setting))
        fields.append(FieldDefinition.End(builder))
    field_vector = _vector(builder, fields)
    index_name = builder.CreateString(value.name) if value.name is not None else None
    IndexDefinition.Start(builder)
    if index_name is not None:
        IndexDefinition.AddName(builder, index_name)
    if field_vector is not None:
        IndexDefinition.AddFields(builder, field_vector)
    return IndexDefinition.End(builder)


def _payload(builder: flatbuffers.Builder, operation: str, request: Any) -> tuple[int, int]:
    if operation in {"insert", "upsert"}:
        module = InsertRequest if operation == "insert" else UpsertRequest
        document = _document(builder, request.document)
        condition = builder.CreateString(request.condition) if operation == "upsert" and request.condition else None
        module.Start(builder)
        if document is not None:
            module.AddDocument(builder, document)
        if condition is not None:
            module.AddCondition(builder, condition)
        payload = module.End(builder)
        tag = RequestPayload.SearchKernel_V2_InsertRequest if operation == "insert" else RequestPayload.SearchKernel_V2_UpsertRequest
    elif operation == "delete":
        identifier = builder.CreateString(request.id)
        condition = builder.CreateString(request.condition) if request.condition else None
        DeleteRequest.Start(builder)
        DeleteRequest.AddId(builder, identifier)
        if condition is not None:
            DeleteRequest.AddCondition(builder, condition)
        payload, tag = DeleteRequest.End(builder), RequestPayload.SearchKernel_V2_DeleteRequest
    elif operation == "get":
        identifier = builder.CreateString(request.id)
        projection = None
        if request.projection is not None:
            fields = _strings(builder, request.projection.fields)
            FieldProjection.Start(builder)
            if fields is not None:
                FieldProjection.AddFields(builder, fields)
            projection = FieldProjection.End(builder)
        GetRequest.Start(builder)
        GetRequest.AddId(builder, identifier)
        if projection is not None:
            GetRequest.AddProjection(builder, projection)
        payload, tag = GetRequest.End(builder), RequestPayload.SearchKernel_V2_GetRequest
    elif operation == "search":
        payload, tag = _search(builder, request), RequestPayload.SearchKernel_V2_SearchRequest
    elif operation == "analyze":
        target = None
        target_type = AnalyzeTarget.NONE
        if request.target is not None and request.target.kind == "field":
            field = builder.CreateString(str(request.target.value))
            AnalyzeFieldTarget.Start(builder)
            AnalyzeFieldTarget.AddField(builder, field)
            target = AnalyzeFieldTarget.End(builder)
            target_type = AnalyzeTarget.AnalyzeFieldTarget
        text = builder.CreateString(request.text) if request.text is not None else None
        language = builder.CreateString(request.language) if request.language is not None else None
        AnalyzeRequest.Start(builder)
        if target is not None:
            AnalyzeRequest.AddTargetType(builder, target_type)
            AnalyzeRequest.AddTarget(builder, target)
        if text is not None:
            AnalyzeRequest.AddText(builder, text)
        if language is not None:
            AnalyzeRequest.AddLanguage(builder, language)
        payload, tag = AnalyzeRequest.End(builder), RequestPayload.SearchKernel_V2_AnalyzeRequest
    elif operation == "describe":
        DescribeRequest.Start(builder)
        payload, tag = DescribeRequest.End(builder), RequestPayload.SearchKernel_V2_DescribeRequest
    elif operation == "inspect_query":
        request_payload = _search(builder, request.request)
        InspectQueryRequest.Start(builder)
        InspectQueryRequest.AddRequest(builder, request_payload)
        payload, tag = InspectQueryRequest.End(builder), RequestPayload.SearchKernel_V2_InspectQueryRequest
    else:
        raise ValueError(f"unsupported operation {operation!r}")
    return tag, payload


def encode_request(operation: str, request: Any) -> bytes:
    builder = flatbuffers.Builder(4096)
    tag, payload = _payload(builder, operation, request)
    RequestEnvelope.Start(builder)
    RequestEnvelope.AddProtocolVersion(builder, PROTOCOL_VERSION)
    RequestEnvelope.AddPayloadType(builder, tag)
    RequestEnvelope.AddPayload(builder, payload)
    envelope = RequestEnvelope.End(builder)
    builder.Finish(envelope, file_identifier=b"SKRQ")
    return bytes(builder.Output())


def encode_index_definition(definition: models.IndexDefinition) -> bytes:
    builder = flatbuffers.Builder(4096)
    root = _index_definition(builder, definition)
    builder.Finish(root)
    return bytes(builder.Output())


def _union(payload: Any, cls: Any) -> Any:
    if not callable(cls):
        cls = getattr(cls, cls.__name__.rsplit(".", 1)[-1])
    value = cls()
    value.Init(payload.Bytes, payload.Pos)
    return value


def _optional_bool(value: Any) -> bool | None:
    return value.Value() if value is not None else None


def _read_field_ref(table: Any) -> models.QueryFieldRef | None:
    if table is None:
        return None
    element = table.ElementIndex()
    return models.QueryFieldRef(
        path=_string(table.Path()),
        element_index=element.Value() if element is not None else None,
    )


def _read_query_source(table: Any) -> models.QuerySource | None:
    if table is None:
        return None
    payload = table.Source()
    if payload is None:
        return models.QuerySource()
    tag = table.SourceType()
    if tag == QuerySourceKind.SearchKernelTextQuery:
        return models.QuerySource(kind="text", text=_string(_union(payload, SearchKernelTextQuery).Text()) or "")
    if tag == QuerySourceKind.MatchAllQuery:
        return models.QuerySource(kind="match_all")
    if tag == QuerySourceKind.YqlQuery:
        from SearchKernel.V2 import YqlQuery

        return models.QuerySource(kind="yql", source=_string(_union(payload, YqlQuery).Source()))
    if tag == QuerySourceKind.TypedQuery:
        return models.QuerySource(kind="typed")
    return models.QuerySource()


def _read_filter_expr(table: Any) -> models.FilterExpr:
    tag = table.ExprType()
    payload = table.Expr()
    if payload is None:
        return models.FilterExpr()
    if tag in (FilterExprKind.FilterAnd, FilterExprKind.FilterOr):
        module = FilterAnd if tag == FilterExprKind.FilterAnd else FilterOr
        item = _union(payload, module)
        return models.FilterExpr(
            kind="and" if tag == FilterExprKind.FilterAnd else "or",
            clauses=[_read_filter_expr(item.Clauses(i)) for i in range(item.ClausesLength())],
        )
    if tag == FilterExprKind.FilterNot:
        item = _union(payload, FilterNot)
        return models.FilterExpr(kind="not", clause=_read_filter_expr(item.Clause()))
    if tag == FilterExprKind.FilterExists:
        item = _union(payload, FilterExists)
        return models.FilterExpr(kind="exists", field=_read_field_ref(item.Field()))
    if tag == FilterExprKind.FilterComparison:
        item = _union(payload, FilterComparison)
        operators = {0: "unspecified", 1: "eq", 2: "ne", 3: "lt", 4: "lte", 5: "gt", 6: "gte"}
        return models.FilterExpr(
            kind="comparison",
            field=_read_field_ref(item.Field()),
            op=operators.get(item.Op(), "unspecified"),
            value=_read_value(item.Value()) if item.Value() is not None else None,
        )
    if tag == FilterExprKind.FilterIn:
        item = _union(payload, FilterIn)
        return models.FilterExpr(
            kind="in",
            field=_read_field_ref(item.Field()),
            values=[_read_value(item.Values(i)) for i in range(item.ValuesLength())],
        )
    return models.FilterExpr()


def _read_resolved_sort_target(table: Any) -> models.ResolvedSortTarget | None:
    if table is None:
        return None
    payload = table.Target()
    if payload is None:
        return models.ResolvedSortTarget()
    if table.TargetType() == ResolvedSortTargetKind.FieldSortTarget:
        return models.ResolvedSortTarget(kind="field", field=_string(_union(payload, FieldSortTarget).Field()))
    if table.TargetType() == ResolvedSortTargetKind.RankFeatureSortTarget:
        from SearchKernel.V2 import RankFeatureSortTarget

        return models.ResolvedSortTarget(kind="rank_feature", feature=_string(_union(payload, RankFeatureSortTarget).Feature()))
    return models.ResolvedSortTarget()


def _read_resolved_sort(table: Any) -> models.ResolvedSortExpr:
    directions = {0: "unspecified", 1: "asc", 2: "desc"}
    return models.ResolvedSortExpr(
        target=_read_resolved_sort_target(table.Target()),
        direction=directions.get(table.Direction(), "unspecified"),
        annotations={
            _string(entry.Key()) or "": _read_value(entry.Value())
            for i in range(table.AnnotationsLength())
            if (entry := table.Annotations(i)) is not None and entry.Value() is not None
        },
    )


def _read_resolved_request(table: Any) -> models.ResolvedSearchRequest:
    return models.ResolvedSearchRequest(
        query=_read_query_source(table.Query()),
        search_fields=[
            models.SearchField(
                field=_string(item.Field()) or "",
                weight=item.Weight().Value() if item.Weight() is not None else None,
            )
            for i in range(table.SearchFieldsLength())
            if (item := table.SearchFields(i)) is not None
        ],
        filters=[
            _read_filter_expr(table.Filters(i))
            for i in range(table.FiltersLength())
            if table.Filters(i) is not None
        ],
        sort=[
            _read_resolved_sort(table.Sort(i))
            for i in range(table.SortLength())
            if table.Sort(i) is not None
        ],
        limit=table.Limit(),
        offset=table.Offset(),
        timeout_ms=table.TimeoutMs().Value() if table.TimeoutMs() is not None else None,
        properties=_read_entries(table) if table.PropertiesLength() else {},
        query_profile=_string(table.QueryProfile()),
        search_chain=_string(table.SearchChain()),
    )


def _read_value(value: Any) -> models.Value:
    tag = value.KindType()
    payload = value.Kind()
    if payload is None:
        raise ValueError("value payload is missing")
    if tag == ValueKind.NullValueValue:
        return models.Value.null()
    if tag == ValueKind.BoolValue:
        return models.Value.boolean(_union(payload, BoolValue).Value())
    if tag == ValueKind.Int64Value:
        return models.Value.int64(_union(payload, Int64Value).Value())
    if tag == ValueKind.UInt64Value:
        return models.Value.uint64(_union(payload, UInt64Value).Value())
    if tag == ValueKind.Float64Value:
        return models.Value.float64(_union(payload, Float64Value).Value())
    if tag == ValueKind.StringValue:
        return models.Value.string(_string(_union(payload, StringValue).Value()) or "")
    if tag == ValueKind.BytesValue:
        item = _union(payload, BytesValue)
        if hasattr(item, "ValueAsNumpy"):
            return models.Value.bytes(bytes(item.ValueAsNumpy()))
        return models.Value.bytes(bytes(item.Value(i) for i in range(item.ValueLength())))
    if tag == ValueKind.ValueList:
        item = _union(payload, ValueList)
        return models.Value.list([_read_value(item.Values(i)) for i in range(item.ValuesLength())])
    if tag == ValueKind.ValueObject:
        item = _union(payload, ValueObject)
        return models.Value.object(_read_entries(item))
    raise ValueError(f"unknown value discriminator {tag}")


def _read_entries(table: Any) -> dict[str, models.Value]:
    result = {}
    for index in range(table.FieldsLength() if hasattr(table, "FieldsLength") else table.PropertiesLength()):
        entry = table.Fields(index) if hasattr(table, "Fields") else table.Properties(index)
        result[_string(entry.Key()) or ""] = _read_value(entry.Value())
    return result


def _read_document(table: Any) -> models.DocumentResult:
    return models.DocumentResult(id=_string(table.Id()) or "", fields=_read_entries(table))


def _read_result_node(table: Any) -> models.ResultNode:
    tag = table.NodeType()
    payload = table.Node()
    if payload is None:
        return models.ResultNode()
    if tag == ResultNodeKind.Hit:
        hit = _union(payload, Hit)
        return models.ResultNode(
            kind="hit",
            id=_string(hit.Id()) or "",
            fields=_read_entries(hit),
            source=_string(hit.Source()),
            types=[_string(hit.Types(i)) or "" for i in range(hit.TypesLength())],
        )
    if tag == ResultNodeKind.ResultGroup:
        group = _union(payload, ResultGroup)
        return models.ResultNode(
            kind="group",
            id=_string(group.Id()),
            label=_string(group.Label()),
            value=_read_value(group.Value()) if group.Value() is not None else None,
            fields=_read_entries(group),
            children=[_read_result_node(group.Children(i)) for i in range(group.ChildrenLength())],
            source=_string(group.Source()),
            types=[_string(group.Types(i)) or "" for i in range(group.TypesLength())],
        )
    return models.ResultNode()


def _read_index(table: Any) -> models.IndexDefinition:
    fields = []
    for index in range(table.FieldsLength()):
        field = table.Fields(index)
        data = field.DataType()
        payload = _union(data.Kind(), PrimitiveDataType) if data is not None and data.Kind() is not None else None
        kind = {
            PrimitiveDataTypeKind.String: "string",
            PrimitiveDataTypeKind.Bool: "bool",
            PrimitiveDataTypeKind.Byte: "byte",
            PrimitiveDataTypeKind.Int32: "int32",
            PrimitiveDataTypeKind.Int64: "int64",
            PrimitiveDataTypeKind.Float32: "float32",
            PrimitiveDataTypeKind.Float64: "float64",
            PrimitiveDataTypeKind.Raw: "raw",
        }.get(payload.Kind() if payload is not None else None, "unspecified")
        matching = field.Matching()
        matching_modes = {
            MatchingMode.Unspecified: "unspecified",
            MatchingMode.Text: "text",
            MatchingMode.Word: "word",
            MatchingMode.Exact: "exact",
            MatchingMode.Gram: "gram",
        }
        matching_mode = matching_modes.get(matching.Mode(), "unspecified") if matching is not None else "unspecified"
        if kind == "string" and matching_mode == "exact":
            kind = "keyword"
        elif kind == "string" and matching_mode == "text":
            kind = "text"
        fields.append(models.FieldDefinition(
            name=_string(field.Name()) or "",
            data_type=models.DataType(kind="primitive", payload_kind=kind),
            stored=_optional_bool(field.Stored()),
            searchable=_optional_bool(field.Searchable()),
            filterable=_optional_bool(field.Filterable()),
            sortable=_optional_bool(field.Sortable()),
            facetable=_optional_bool(field.Facetable()),
            matching=(models.MatchingConfig(mode=matching_mode)
                      if matching is not None else None),
        ))
    default_fields = table.DefaultSearchFields()
    defaults = None
    if default_fields is not None:
        defaults = models.SearchFieldSelection(
            fields=[
                models.SearchField(
                    field=_string(item.Field()) or "",
                    weight=item.Weight().Value() if item.Weight() is not None else None,
                )
                for i in range(default_fields.FieldsLength())
                if (item := default_fields.Fields(i)) is not None
            ]
        )
    return models.IndexDefinition(
        name=_string(table.Name()),
        fields=fields,
        default_search_fields=defaults,
        properties=_read_entries(table) if table.PropertiesLength() else {},
        document_type=_string(table.DocumentType()),
        schema_inherits=[_string(table.SchemaInherits(i)) or "" for i in range(table.SchemaInheritsLength())],
        document_inherits=[_string(table.DocumentInherits(i)) or "" for i in range(table.DocumentInheritsLength())],
    )


def _read_response(data: bytes, operation: str) -> Any:
    if not ResponseEnvelope.ResponseEnvelope.ResponseEnvelopeBufferHasIdentifier(data, 0):
        raise ValueError("response is not a SKRS envelope")
    envelope = ResponseEnvelope.ResponseEnvelope.GetRootAs(data, 0)
    if envelope.ProtocolVersion() != PROTOCOL_VERSION:
        raise ValueError("unsupported response protocol version")
    payload = envelope.Payload()
    if payload is None:
        raise ValueError("response payload is missing")
    tag = envelope.PayloadType()
    if tag == ResponsePayload.SearchKernel_V2_Error:
        error = _union(payload, Error)
        details = []
        for index in range(error.DetailsLength()):
            detail = error.Details(index)
            if detail is not None:
                details.append(
                    models.ErrorDetail(
                        code=_string(detail.Code()) or "",
                        path=_string(detail.Path()),
                        message=_string(detail.Message()) or "",
                        metadata={
                            _string(entry.Key()) or "": _string(entry.Value()) or ""
                            for i in range(detail.MetadataLength())
                            if (entry := detail.Metadata(i)) is not None
                        },
                    )
                )
        error_codes = {
            0: "unspecified",
            1: "invalid_argument",
            2: "not_found",
            3: "already_exists",
            4: "failed_precondition",
            5: "conflict",
            6: "unsupported",
            7: "resource_exhausted",
            8: "internal",
            9: "unavailable",
            10: "deadline_exceeded",
        }
        raise PortableResponseError(models.Error(code=error_codes.get(error.Code(), "unspecified"),
                                                  message=_string(error.Message()) or "", details=details,
                                                  retryable=error.Retryable()))
    if operation == "insert":
        return models.InsertResult(id=_string(_union(payload, InsertResult).Id()) or "")
    if operation == "upsert":
        return models.UpsertResult(id=_string(_union(payload, UpsertResult).Id()) or "")
    if operation == "delete":
        item = _union(payload, DeleteResult)
        return models.DeleteResult(id=_string(item.Id()) or "", deleted=item.Deleted())
    if operation == "get":
        item = _union(payload, GetResult)
        document = item.Document()
        return models.GetResult(document=_read_document(document) if document is not None else None)
    if operation == "analyze":
        item = _union(payload, AnalyzeResult)
        return models.AnalyzeResult(tokens=[models.AnalyzedToken(surface=_string((token := item.Tokens(i)).Surface()) or "", term=_string(token.Term()) or "", position=token.Position(), start_byte=token.StartByte(), end_byte=token.EndByte()) for i in range(item.TokensLength())])
    if operation == "describe":
        item = _union(payload, DescribeResult)
        index = item.Index()
        return models.DescribeResult(index=_read_index(index) if index is not None else None)
    if operation == "inspect_query":
        item = _union(payload, InspectQueryResult)
        resolved = item.ResolvedRequest()
        return models.InspectQueryResult(
            resolved_request=_read_resolved_request(resolved) if resolved is not None else None
        )
    if operation == "search":
        item = _union(payload, SearchResult)
        total = item.TotalHits()
        relations = {
            TotalHitsRelation.Unspecified: "unspecified",
            TotalHitsRelation.Exact: "exact",
            TotalHitsRelation.LowerBound: "lower_bound",
            TotalHitsRelation.Approximate: "approximate",
            TotalHitsRelation.Unknown: "unknown",
        }
        return models.SearchResult(root=_read_result_node(item.Root()) if item.Root() is not None else None,
                                   total_hits=models.TotalHits(value=total.Value(), relation=relations.get(total.Relation(), "unspecified")) if total else None)
    raise ValueError(f"unexpected response operation {operation!r}")


class PortableResponseError(ValueError):
    """A canonical Search Kernel Error response."""

    def __init__(self, error: models.Error):
        super().__init__(error.message)
        self.error = error


def decode_response(data: bytes, operation: str) -> Any:
    return _read_response(data, operation)
