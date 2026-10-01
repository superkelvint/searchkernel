from __future__ import annotations

import flatbuffers
import numpy as np

import typing
from typing import cast

uoffset: typing.TypeAlias = flatbuffers.number_types.UOffsetTFlags.py_type

class ArithmeticUpdateOperator(object):
  Unspecified = cast(int, ...)
  Increment = cast(int, ...)
  Decrement = cast(int, ...)
  Multiply = cast(int, ...)
  Divide = cast(int, ...)
