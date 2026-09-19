from __future__ import annotations

import flatbuffers
import numpy as np

import typing
from typing import cast

uoffset: typing.TypeAlias = flatbuffers.number_types.UOffsetTFlags.py_type

class RelevanceKind(object):
  NONE = cast(int, ...)
  FiniteRelevance = cast(int, ...)
  NegativeInfinityRelevance = cast(int, ...)
  PositiveInfinityRelevance = cast(int, ...)

