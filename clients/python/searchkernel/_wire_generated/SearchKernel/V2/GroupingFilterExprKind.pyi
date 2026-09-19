from __future__ import annotations

import flatbuffers
import numpy as np

import typing
from typing import cast

uoffset: typing.TypeAlias = flatbuffers.number_types.UOffsetTFlags.py_type

class GroupingFilterExprKind(object):
  NONE = cast(int, ...)
  GroupingFilterAnd = cast(int, ...)
  GroupingFilterOr = cast(int, ...)
  GroupingFilterNot = cast(int, ...)
  GroupingFilterRegex = cast(int, ...)
  GroupingFilterRange = cast(int, ...)
  GroupingFilterIsTrue = cast(int, ...)

