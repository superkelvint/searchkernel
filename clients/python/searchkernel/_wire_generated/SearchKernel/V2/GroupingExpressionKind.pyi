from __future__ import annotations

import flatbuffers
import numpy as np

import typing
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

