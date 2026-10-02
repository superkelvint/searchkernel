from __future__ import annotations

import flatbuffers
import numpy as np

import typing
from typing import cast

uoffset: typing.TypeAlias = flatbuffers.number_types.UOffsetTFlags.py_type

class GroupingBinaryOperator(object):
  Unspecified = cast(int, ...)
  Add = cast(int, ...)
  Subtract = cast(int, ...)
  Multiply = cast(int, ...)
  Divide = cast(int, ...)
  Modulo = cast(int, ...)
