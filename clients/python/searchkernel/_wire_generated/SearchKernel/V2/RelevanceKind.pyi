from __future__ import annotations

import flatbuffers
import numpy as np

import typing
from SearchKernel.V2.FiniteRelevance import FiniteRelevance
from SearchKernel.V2.NegativeInfinityRelevance import NegativeInfinityRelevance
from SearchKernel.V2.PositiveInfinityRelevance import PositiveInfinityRelevance
from flatbuffers import table
from typing import cast

uoffset: typing.TypeAlias = flatbuffers.number_types.UOffsetTFlags.py_type

class RelevanceKind(object):
  NONE = cast(int, ...)
  FiniteRelevance = cast(int, ...)
  NegativeInfinityRelevance = cast(int, ...)
  PositiveInfinityRelevance = cast(int, ...)
def RelevanceKindCreator(union_type: typing.Literal[RelevanceKind.NONE, RelevanceKind.FiniteRelevance, RelevanceKind.NegativeInfinityRelevance, RelevanceKind.PositiveInfinityRelevance], table: table.Table) -> typing.Union[None, FiniteRelevance, NegativeInfinityRelevance, PositiveInfinityRelevance]: ...

