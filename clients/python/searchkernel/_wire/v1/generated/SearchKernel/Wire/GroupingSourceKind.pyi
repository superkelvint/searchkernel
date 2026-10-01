from __future__ import annotations

import flatbuffers
import numpy as np

import typing
from SearchKernel.Wire.RawGroupingSource import RawGroupingSource
from SearchKernel.Wire.StructuredGroupingSource import StructuredGroupingSource
from flatbuffers import table
from typing import cast

uoffset: typing.TypeAlias = flatbuffers.number_types.UOffsetTFlags.py_type

class GroupingSourceKind(object):
  NONE = cast(int, ...)
  RawGroupingSource = cast(int, ...)
  StructuredGroupingSource = cast(int, ...)
def GroupingSourceKindCreator(union_type: typing.Literal[GroupingSourceKind.NONE, GroupingSourceKind.RawGroupingSource, GroupingSourceKind.StructuredGroupingSource], table: table.Table) -> typing.Union[None, RawGroupingSource, StructuredGroupingSource]: ...
