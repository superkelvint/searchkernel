from __future__ import annotations

import flatbuffers
import numpy as np

import typing
from SearchKernel.V2.AddUpdate import AddUpdate
from SearchKernel.V2.ArithmeticUpdate import ArithmeticUpdate
from SearchKernel.V2.AssignUpdate import AssignUpdate
from SearchKernel.V2.ClearUpdate import ClearUpdate
from SearchKernel.V2.MatchUpdate import MatchUpdate
from SearchKernel.V2.RemoveUpdate import RemoveUpdate
from SearchKernel.V2.TensorModifyUpdate import TensorModifyUpdate
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

