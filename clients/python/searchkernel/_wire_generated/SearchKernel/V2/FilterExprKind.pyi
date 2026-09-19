from __future__ import annotations

import flatbuffers
import numpy as np

import typing
from SearchKernel.V2.FilterAnd import FilterAnd
from SearchKernel.V2.FilterComparison import FilterComparison
from SearchKernel.V2.FilterExists import FilterExists
from SearchKernel.V2.FilterIn import FilterIn
from SearchKernel.V2.FilterNot import FilterNot
from SearchKernel.V2.FilterOr import FilterOr
from SearchKernel.V2.FilterRange import FilterRange
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

