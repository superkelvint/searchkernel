from __future__ import annotations

import flatbuffers
import numpy as np

import typing
from SearchKernel.Wire.FieldSortTarget import FieldSortTarget
from SearchKernel.Wire.RankFeatureSortTarget import RankFeatureSortTarget
from SearchKernel.Wire.RawSortTarget import RawSortTarget
from SearchKernel.Wire.RelevanceSortTarget import RelevanceSortTarget
from flatbuffers import table
from typing import cast

uoffset: typing.TypeAlias = flatbuffers.number_types.UOffsetTFlags.py_type

class SortTargetKind(object):
  NONE = cast(int, ...)
  FieldSortTarget = cast(int, ...)
  RelevanceSortTarget = cast(int, ...)
  RankFeatureSortTarget = cast(int, ...)
  RawSortTarget = cast(int, ...)
def SortTargetKindCreator(union_type: typing.Literal[SortTargetKind.NONE, SortTargetKind.FieldSortTarget, SortTargetKind.RelevanceSortTarget, SortTargetKind.RankFeatureSortTarget, SortTargetKind.RawSortTarget], table: table.Table) -> typing.Union[None, FieldSortTarget, RelevanceSortTarget, RankFeatureSortTarget, RawSortTarget]: ...
