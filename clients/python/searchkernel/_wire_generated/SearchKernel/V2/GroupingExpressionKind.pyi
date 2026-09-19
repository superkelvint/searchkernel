from __future__ import annotations

import flatbuffers
import numpy as np

import typing
from SearchKernel.V2.GroupingAggregationExpression import GroupingAggregationExpression
from SearchKernel.V2.GroupingAliasReference import GroupingAliasReference
from SearchKernel.V2.GroupingAttributeExpression import GroupingAttributeExpression
from SearchKernel.V2.GroupingBinaryExpression import GroupingBinaryExpression
from SearchKernel.V2.GroupingBucketExpression import GroupingBucketExpression
from SearchKernel.V2.GroupingFunctionExpression import GroupingFunctionExpression
from SearchKernel.V2.GroupingLiteralExpression import GroupingLiteralExpression
from SearchKernel.V2.GroupingMapLookupExpression import GroupingMapLookupExpression
from SearchKernel.V2.GroupingMemberExpression import GroupingMemberExpression
from SearchKernel.V2.GroupingUnaryExpression import GroupingUnaryExpression
from flatbuffers import table
from typing import cast

uoffset: typing.TypeAlias = flatbuffers.number_types.UOffsetTFlags.py_type

class GroupingExpressionKind(object):
  NONE = cast(int, ...)
  GroupingAttributeExpression = cast(int, ...)
  GroupingLiteralExpression = cast(int, ...)
  GroupingFunctionExpression = cast(int, ...)
  GroupingUnaryExpression = cast(int, ...)
  GroupingBinaryExpression = cast(int, ...)
  GroupingAliasReference = cast(int, ...)
  GroupingMemberExpression = cast(int, ...)
  GroupingMapLookupExpression = cast(int, ...)
  GroupingBucketExpression = cast(int, ...)
  GroupingAggregationExpression = cast(int, ...)
def GroupingExpressionKindCreator(union_type: typing.Literal[GroupingExpressionKind.NONE, GroupingExpressionKind.GroupingAttributeExpression, GroupingExpressionKind.GroupingLiteralExpression, GroupingExpressionKind.GroupingFunctionExpression, GroupingExpressionKind.GroupingUnaryExpression, GroupingExpressionKind.GroupingBinaryExpression, GroupingExpressionKind.GroupingAliasReference, GroupingExpressionKind.GroupingMemberExpression, GroupingExpressionKind.GroupingMapLookupExpression, GroupingExpressionKind.GroupingBucketExpression, GroupingExpressionKind.GroupingAggregationExpression], table: table.Table) -> typing.Union[None, GroupingAttributeExpression, GroupingLiteralExpression, GroupingFunctionExpression, GroupingUnaryExpression, GroupingBinaryExpression, GroupingAliasReference, GroupingMemberExpression, GroupingMapLookupExpression, GroupingBucketExpression, GroupingAggregationExpression]: ...

