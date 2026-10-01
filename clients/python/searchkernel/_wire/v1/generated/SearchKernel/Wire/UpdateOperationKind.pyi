from __future__ import annotations

import flatbuffers
import numpy as np

import typing
from SearchKernel.Wire.AddUpdate import AddUpdate
from SearchKernel.Wire.ArithmeticUpdate import ArithmeticUpdate
from SearchKernel.Wire.AssignUpdate import AssignUpdate
from SearchKernel.Wire.ClearUpdate import ClearUpdate
from SearchKernel.Wire.MatchUpdate import MatchUpdate
from SearchKernel.Wire.RemoveUpdate import RemoveUpdate
from SearchKernel.Wire.TensorModifyUpdate import TensorModifyUpdate
from flatbuffers import table
from typing import cast

uoffset: typing.TypeAlias = flatbuffers.number_types.UOffsetTFlags.py_type

class UpdateOperationKind(object):
  NONE = cast(int, ...)
  AssignUpdate = cast(int, ...)
  ClearUpdate = cast(int, ...)
  AddUpdate = cast(int, ...)
  RemoveUpdate = cast(int, ...)
  ArithmeticUpdate = cast(int, ...)
  MatchUpdate = cast(int, ...)
  TensorModifyUpdate = cast(int, ...)
def UpdateOperationKindCreator(union_type: typing.Literal[UpdateOperationKind.NONE, UpdateOperationKind.AssignUpdate, UpdateOperationKind.ClearUpdate, UpdateOperationKind.AddUpdate, UpdateOperationKind.RemoveUpdate, UpdateOperationKind.ArithmeticUpdate, UpdateOperationKind.MatchUpdate, UpdateOperationKind.TensorModifyUpdate], table: table.Table) -> typing.Union[None, AssignUpdate, ClearUpdate, AddUpdate, RemoveUpdate, ArithmeticUpdate, MatchUpdate, TensorModifyUpdate]: ...
