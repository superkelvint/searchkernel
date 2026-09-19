from __future__ import annotations

import flatbuffers
import numpy as np

import typing
from typing import cast

uoffset: typing.TypeAlias = flatbuffers.number_types.UOffsetTFlags.py_type

class GroupingOperationKind(object):
  NONE = cast(int, ...)
  GroupingAllOperation = cast(int, ...)
  GroupingEachOperation = cast(int, ...)
  GroupingGroupOperation = cast(int, ...)
  GroupingOutputOperation = cast(int, ...)
  GroupingMaxOperation = cast(int, ...)
  GroupingPrecisionOperation = cast(int, ...)
  GroupingOrderOperation = cast(int, ...)
  GroupingAliasOperation = cast(int, ...)
  GroupingFilterOperation = cast(int, ...)
  GroupingKeepOperation = cast(int, ...)
  GroupingWhereOperation = cast(int, ...)

