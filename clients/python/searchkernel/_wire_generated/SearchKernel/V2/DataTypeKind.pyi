from __future__ import annotations

import flatbuffers
import numpy as np

import typing
from typing import cast

uoffset: typing.TypeAlias = flatbuffers.number_types.UOffsetTFlags.py_type

class DataTypeKind(object):
  NONE = cast(int, ...)
  PrimitiveDataType = cast(int, ...)
  TensorDataType = cast(int, ...)
  ArrayDataType = cast(int, ...)
  MapDataType = cast(int, ...)
  WeightedSetDataType = cast(int, ...)
  StructDataType = cast(int, ...)
  StructReferenceDataType = cast(int, ...)
  PredicateDataType = cast(int, ...)
  PositionDataType = cast(int, ...)
  ReferenceDataType = cast(int, ...)

