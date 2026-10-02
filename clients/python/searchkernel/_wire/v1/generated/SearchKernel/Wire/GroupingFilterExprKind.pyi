from __future__ import annotations

import flatbuffers
import numpy as np

import typing
from SearchKernel.Wire.GroupingFilterAnd import GroupingFilterAnd
from SearchKernel.Wire.GroupingFilterIsTrue import GroupingFilterIsTrue
from SearchKernel.Wire.GroupingFilterNot import GroupingFilterNot
from SearchKernel.Wire.GroupingFilterOr import GroupingFilterOr
from SearchKernel.Wire.GroupingFilterRange import GroupingFilterRange
from SearchKernel.Wire.GroupingFilterRegex import GroupingFilterRegex
from flatbuffers import table
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
def GroupingFilterExprKindCreator(union_type: typing.Literal[GroupingFilterExprKind.NONE, GroupingFilterExprKind.GroupingFilterAnd, GroupingFilterExprKind.GroupingFilterOr, GroupingFilterExprKind.GroupingFilterNot, GroupingFilterExprKind.GroupingFilterRegex, GroupingFilterExprKind.GroupingFilterRange, GroupingFilterExprKind.GroupingFilterIsTrue], table: table.Table) -> typing.Union[None, GroupingFilterAnd, GroupingFilterOr, GroupingFilterNot, GroupingFilterRegex, GroupingFilterRange, GroupingFilterIsTrue]: ...
