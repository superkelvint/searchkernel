from __future__ import annotations

import flatbuffers
import numpy as np

import typing
from typing import cast

uoffset: typing.TypeAlias = flatbuffers.number_types.UOffsetTFlags.py_type

class RangeBoundKind(object):
  NONE = cast(int, ...)
  FiniteRangeBound = cast(int, ...)
  NegativeInfinityBound = cast(int, ...)
  PositiveInfinityBound = cast(int, ...)
  UnboundedRangeBound = cast(int, ...)

