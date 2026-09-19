from __future__ import annotations

import flatbuffers
import numpy as np

import typing
from typing import cast

uoffset: typing.TypeAlias = flatbuffers.number_types.UOffsetTFlags.py_type

class MatchedElementUpdateKind(object):
  NONE = cast(int, ...)
  AssignUpdate = cast(int, ...)
  ClearUpdate = cast(int, ...)
  ArithmeticUpdate = cast(int, ...)

