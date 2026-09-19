from __future__ import annotations

import flatbuffers
import numpy as np

import typing
from SearchKernel.V2.FiniteRangeBound import FiniteRangeBound
from SearchKernel.V2.NegativeInfinityBound import NegativeInfinityBound
from SearchKernel.V2.PositiveInfinityBound import PositiveInfinityBound
from SearchKernel.V2.UnboundedRangeBound import UnboundedRangeBound
from flatbuffers import table
from typing import cast

uoffset: typing.TypeAlias = flatbuffers.number_types.UOffsetTFlags.py_type

class RangeBoundKind(object):
  NONE = cast(int, ...)
  FiniteRangeBound = cast(int, ...)
  NegativeInfinityBound = cast(int, ...)
  PositiveInfinityBound = cast(int, ...)
  UnboundedRangeBound = cast(int, ...)
def RangeBoundKindCreator(union_type: typing.Literal[RangeBoundKind.NONE, RangeBoundKind.FiniteRangeBound, RangeBoundKind.NegativeInfinityBound, RangeBoundKind.PositiveInfinityBound, RangeBoundKind.UnboundedRangeBound], table: table.Table) -> typing.Union[None, FiniteRangeBound, NegativeInfinityBound, PositiveInfinityBound, UnboundedRangeBound]: ...

