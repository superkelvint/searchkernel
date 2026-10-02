from __future__ import annotations

import flatbuffers
import numpy as np

import typing
from typing import cast

uoffset: typing.TypeAlias = flatbuffers.number_types.UOffsetTFlags.py_type

class TensorModifyOperator(object):
  Unspecified = cast(int, ...)
  Replace = cast(int, ...)
  Add = cast(int, ...)
  Multiply = cast(int, ...)
