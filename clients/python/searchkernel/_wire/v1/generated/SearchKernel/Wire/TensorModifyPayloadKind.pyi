from __future__ import annotations

import flatbuffers
import numpy as np

import typing
from SearchKernel.Wire.MixedTensorBlocks import MixedTensorBlocks
from SearchKernel.Wire.SparseTensorCells import SparseTensorCells
from flatbuffers import table
from typing import cast

uoffset: typing.TypeAlias = flatbuffers.number_types.UOffsetTFlags.py_type

class TensorModifyPayloadKind(object):
  NONE = cast(int, ...)
  SparseTensorCells = cast(int, ...)
  MixedTensorBlocks = cast(int, ...)
def TensorModifyPayloadKindCreator(union_type: typing.Literal[TensorModifyPayloadKind.NONE, TensorModifyPayloadKind.SparseTensorCells, TensorModifyPayloadKind.MixedTensorBlocks], table: table.Table) -> typing.Union[None, SparseTensorCells, MixedTensorBlocks]: ...
