from __future__ import annotations

import flatbuffers
import numpy as np

import typing
from SearchKernel.V2.ArrayDataType import ArrayDataType
from SearchKernel.V2.MapDataType import MapDataType
from SearchKernel.V2.PositionDataType import PositionDataType
from SearchKernel.V2.PredicateDataType import PredicateDataType
from SearchKernel.V2.PrimitiveDataType import PrimitiveDataType
from SearchKernel.V2.ReferenceDataType import ReferenceDataType
from SearchKernel.V2.StructDataType import StructDataType
from SearchKernel.V2.StructReferenceDataType import StructReferenceDataType
from SearchKernel.V2.TensorDataType import TensorDataType
from SearchKernel.V2.WeightedSetDataType import WeightedSetDataType
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

