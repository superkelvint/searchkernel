from __future__ import annotations

import flatbuffers
import numpy as np

import typing
from typing import cast

uoffset: typing.TypeAlias = flatbuffers.number_types.UOffsetTFlags.py_type

class PrimitiveDataTypeKind(object):
  Unspecified = cast(int, ...)
  String = cast(int, ...)
  Bool = cast(int, ...)
  Byte = cast(int, ...)
  Int32 = cast(int, ...)
  Int64 = cast(int, ...)
  Float32 = cast(int, ...)
  Float64 = cast(int, ...)
  Raw = cast(int, ...)
