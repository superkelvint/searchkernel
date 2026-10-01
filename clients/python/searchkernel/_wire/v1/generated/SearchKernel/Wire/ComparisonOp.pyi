from __future__ import annotations

import flatbuffers
import numpy as np

import typing
from typing import cast

uoffset: typing.TypeAlias = flatbuffers.number_types.UOffsetTFlags.py_type

class ComparisonOp(object):
  Unspecified = cast(int, ...)
  Eq = cast(int, ...)
  Ne = cast(int, ...)
  Lt = cast(int, ...)
  Lte = cast(int, ...)
  Gt = cast(int, ...)
  Gte = cast(int, ...)
