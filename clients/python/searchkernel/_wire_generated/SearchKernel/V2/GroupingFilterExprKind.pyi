from __future__ import annotations

import flatbuffers
import numpy as np

import typing
from SearchKernel.V2.GroupingFilterAnd import GroupingFilterAnd
from SearchKernel.V2.GroupingFilterIsTrue import GroupingFilterIsTrue
from SearchKernel.V2.GroupingFilterNot import GroupingFilterNot
from SearchKernel.V2.GroupingFilterOr import GroupingFilterOr
from SearchKernel.V2.GroupingFilterRange import GroupingFilterRange
from SearchKernel.V2.GroupingFilterRegex import GroupingFilterRegex
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

