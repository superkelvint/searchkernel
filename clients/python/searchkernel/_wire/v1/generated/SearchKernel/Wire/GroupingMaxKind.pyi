from __future__ import annotations

import flatbuffers
import numpy as np

import typing
from SearchKernel.Wire.FiniteGroupingMax import FiniteGroupingMax
from SearchKernel.Wire.InfiniteGroupingMax import InfiniteGroupingMax
from flatbuffers import table
from typing import cast

uoffset: typing.TypeAlias = flatbuffers.number_types.UOffsetTFlags.py_type

class GroupingMaxKind(object):
  NONE = cast(int, ...)
  FiniteGroupingMax = cast(int, ...)
  InfiniteGroupingMax = cast(int, ...)
def GroupingMaxKindCreator(union_type: typing.Literal[GroupingMaxKind.NONE, GroupingMaxKind.FiniteGroupingMax, GroupingMaxKind.InfiniteGroupingMax], table: table.Table) -> typing.Union[None, FiniteGroupingMax, InfiniteGroupingMax]: ...
