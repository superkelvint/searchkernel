from __future__ import annotations

import flatbuffers
import numpy as np

import typing
from SearchKernel.Wire.FiniteRangeBound import FiniteRangeBound
from SearchKernel.Wire.NegativeInfinityBound import NegativeInfinityBound
from SearchKernel.Wire.PositiveInfinityBound import PositiveInfinityBound
from SearchKernel.Wire.UnboundedRangeBound import UnboundedRangeBound
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
