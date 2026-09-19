from __future__ import annotations

import flatbuffers
import numpy as np

import typing
from typing import cast

uoffset: typing.TypeAlias = flatbuffers.number_types.UOffsetTFlags.py_type

class ValueKind(object):
  NONE = cast(int, ...)
  NullValueValue = cast(int, ...)
  BoolValue = cast(int, ...)
  Int64Value = cast(int, ...)
  UInt64Value = cast(int, ...)
  Float64Value = cast(int, ...)
  NonFiniteFloatValue = cast(int, ...)
  StringValue = cast(int, ...)
  BytesValue = cast(int, ...)
  ValueList = cast(int, ...)
  ValueObject = cast(int, ...)
  TensorValue = cast(int, ...)

