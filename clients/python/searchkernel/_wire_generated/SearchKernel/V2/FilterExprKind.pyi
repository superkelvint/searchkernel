from __future__ import annotations

import flatbuffers
import numpy as np

import typing
from typing import cast

uoffset: typing.TypeAlias = flatbuffers.number_types.UOffsetTFlags.py_type

class FilterExprKind(object):
  NONE = cast(int, ...)
  FilterAnd = cast(int, ...)
  FilterOr = cast(int, ...)
  FilterNot = cast(int, ...)
  FilterComparison = cast(int, ...)
  FilterRange = cast(int, ...)
  FilterExists = cast(int, ...)
  FilterIn = cast(int, ...)

