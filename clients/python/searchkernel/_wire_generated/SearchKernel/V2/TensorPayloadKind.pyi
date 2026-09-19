from __future__ import annotations

import flatbuffers
import numpy as np

import typing
from SearchKernel.V2.DenseTensorValues import DenseTensorValues
from SearchKernel.V2.MixedTensorBlocks import MixedTensorBlocks
from SearchKernel.V2.SparseTensorCells import SparseTensorCells
from flatbuffers import table
from typing import cast

uoffset: typing.TypeAlias = flatbuffers.number_types.UOffsetTFlags.py_type

class TensorPayloadKind(object):
  NONE = cast(int, ...)
  DenseTensorValues = cast(int, ...)
  SparseTensorCells = cast(int, ...)
  MixedTensorBlocks = cast(int, ...)
def TensorPayloadKindCreator(union_type: typing.Literal[TensorPayloadKind.NONE, TensorPayloadKind.DenseTensorValues, TensorPayloadKind.SparseTensorCells, TensorPayloadKind.MixedTensorBlocks], table: table.Table) -> typing.Union[None, DenseTensorValues, SparseTensorCells, MixedTensorBlocks]: ...

