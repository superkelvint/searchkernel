from __future__ import annotations

import flatbuffers
import numpy as np

import typing
from SearchKernel.Wire.ArrayDataType import ArrayDataType
from SearchKernel.Wire.MapDataType import MapDataType
from SearchKernel.Wire.PositionDataType import PositionDataType
from SearchKernel.Wire.PredicateDataType import PredicateDataType
from SearchKernel.Wire.PrimitiveDataType import PrimitiveDataType
from SearchKernel.Wire.ReferenceDataType import ReferenceDataType
from SearchKernel.Wire.StructDataType import StructDataType
from SearchKernel.Wire.StructReferenceDataType import StructReferenceDataType
from SearchKernel.Wire.TensorDataType import TensorDataType
from SearchKernel.Wire.WeightedSetDataType import WeightedSetDataType
from flatbuffers import table
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
def DataTypeKindCreator(union_type: typing.Literal[DataTypeKind.NONE, DataTypeKind.PrimitiveDataType, DataTypeKind.TensorDataType, DataTypeKind.ArrayDataType, DataTypeKind.MapDataType, DataTypeKind.WeightedSetDataType, DataTypeKind.StructDataType, DataTypeKind.StructReferenceDataType, DataTypeKind.PredicateDataType, DataTypeKind.PositionDataType, DataTypeKind.ReferenceDataType], table: table.Table) -> typing.Union[None, PrimitiveDataType, TensorDataType, ArrayDataType, MapDataType, WeightedSetDataType, StructDataType, StructReferenceDataType, PredicateDataType, PositionDataType, ReferenceDataType]: ...
