from __future__ import annotations

import flatbuffers
import numpy as np

import typing
from SearchKernel.Wire.FilterAnd import FilterAnd
from SearchKernel.Wire.FilterComparison import FilterComparison
from SearchKernel.Wire.FilterExists import FilterExists
from SearchKernel.Wire.FilterIn import FilterIn
from SearchKernel.Wire.FilterNot import FilterNot
from SearchKernel.Wire.FilterOr import FilterOr
from SearchKernel.Wire.FilterRange import FilterRange
from flatbuffers import table
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
def FilterExprKindCreator(union_type: typing.Literal[FilterExprKind.NONE, FilterExprKind.FilterAnd, FilterExprKind.FilterOr, FilterExprKind.FilterNot, FilterExprKind.FilterComparison, FilterExprKind.FilterRange, FilterExprKind.FilterExists, FilterExprKind.FilterIn], table: table.Table) -> typing.Union[None, FilterAnd, FilterOr, FilterNot, FilterComparison, FilterRange, FilterExists, FilterIn]: ...
