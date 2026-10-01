from __future__ import annotations

import flatbuffers
import numpy as np

import typing
from typing import cast

uoffset: typing.TypeAlias = flatbuffers.number_types.UOffsetTFlags.py_type

class TotalHitsRelation(object):
  Unspecified = cast(int, ...)
  Exact = cast(int, ...)
  LowerBound = cast(int, ...)
  Approximate = cast(int, ...)
  Unknown = cast(int, ...)
