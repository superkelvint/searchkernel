from __future__ import annotations

import flatbuffers
import numpy as np

import typing
from SearchKernel.Wire.GroupingAggregationExpression import GroupingAggregationExpression
from SearchKernel.Wire.GroupingAliasReference import GroupingAliasReference
from SearchKernel.Wire.GroupingAttributeExpression import GroupingAttributeExpression
from SearchKernel.Wire.GroupingBinaryExpression import GroupingBinaryExpression
from SearchKernel.Wire.GroupingBucketExpression import GroupingBucketExpression
from SearchKernel.Wire.GroupingFunctionExpression import GroupingFunctionExpression
from SearchKernel.Wire.GroupingLiteralExpression import GroupingLiteralExpression
from SearchKernel.Wire.GroupingMapLookupExpression import GroupingMapLookupExpression
from SearchKernel.Wire.GroupingMemberExpression import GroupingMemberExpression
from SearchKernel.Wire.GroupingUnaryExpression import GroupingUnaryExpression
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
