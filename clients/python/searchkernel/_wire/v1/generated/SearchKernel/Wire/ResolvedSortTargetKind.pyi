from __future__ import annotations

import flatbuffers
import numpy as np

import typing
from SearchKernel.Wire.FieldSortTarget import FieldSortTarget
from SearchKernel.Wire.RankFeatureSortTarget import RankFeatureSortTarget
from SearchKernel.Wire.RelevanceSortTarget import RelevanceSortTarget
from flatbuffers import table
from typing import cast

uoffset: typing.TypeAlias = flatbuffers.number_types.UOffsetTFlags.py_type

class ResolvedSortTargetKind(object):
  NONE = cast(int, ...)
  FieldSortTarget = cast(int, ...)
  RelevanceSortTarget = cast(int, ...)
  RankFeatureSortTarget = cast(int, ...)
def ResolvedSortTargetKindCreator(union_type: typing.Literal[ResolvedSortTargetKind.NONE, ResolvedSortTargetKind.FieldSortTarget, ResolvedSortTargetKind.RelevanceSortTarget, ResolvedSortTargetKind.RankFeatureSortTarget], table: table.Table) -> typing.Union[None, FieldSortTarget, RelevanceSortTarget, RankFeatureSortTarget]: ...
