from __future__ import annotations

import flatbuffers
import numpy as np

import typing
from SearchKernel.Wire.BoolValue import BoolValue
from SearchKernel.Wire.BytesValue import BytesValue
from SearchKernel.Wire.Float64Value import Float64Value
from SearchKernel.Wire.Int64Value import Int64Value
from SearchKernel.Wire.NonFiniteFloatValue import NonFiniteFloatValue
from SearchKernel.Wire.NullValueValue import NullValueValue
from SearchKernel.Wire.StringValue import StringValue
from SearchKernel.Wire.TensorValue import TensorValue
from SearchKernel.Wire.UInt64Value import UInt64Value
from SearchKernel.Wire.ValueList import ValueList
from SearchKernel.Wire.ValueObject import ValueObject
from flatbuffers import table
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
def ValueKindCreator(union_type: typing.Literal[ValueKind.NONE, ValueKind.NullValueValue, ValueKind.BoolValue, ValueKind.Int64Value, ValueKind.UInt64Value, ValueKind.Float64Value, ValueKind.NonFiniteFloatValue, ValueKind.StringValue, ValueKind.BytesValue, ValueKind.ValueList, ValueKind.ValueObject, ValueKind.TensorValue], table: table.Table) -> typing.Union[None, NullValueValue, BoolValue, Int64Value, UInt64Value, Float64Value, NonFiniteFloatValue, StringValue, BytesValue, ValueList, ValueObject, TensorValue]: ...
