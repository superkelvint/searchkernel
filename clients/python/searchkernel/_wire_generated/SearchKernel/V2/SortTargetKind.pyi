from __future__ import annotations

import flatbuffers
import numpy as np

import typing
from typing import cast

uoffset: typing.TypeAlias = flatbuffers.number_types.UOffsetTFlags.py_type

class SortTargetKind(object):
  NONE = cast(int, ...)
  FieldSortTarget = cast(int, ...)
  RelevanceSortTarget = cast(int, ...)
  RankFeatureSortTarget = cast(int, ...)
  RawSortTarget = cast(int, ...)

