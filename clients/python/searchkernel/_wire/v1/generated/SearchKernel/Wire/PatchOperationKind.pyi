from __future__ import annotations

import flatbuffers
import numpy as np

import typing
from SearchKernel.Wire.SetField import SetField
from SearchKernel.Wire.UnsetField import UnsetField
from flatbuffers import table
from typing import cast

uoffset: typing.TypeAlias = flatbuffers.number_types.UOffsetTFlags.py_type

class PatchOperationKind(object):
  NONE = cast(int, ...)
  SetField = cast(int, ...)
  UnsetField = cast(int, ...)
def PatchOperationKindCreator(union_type: typing.Literal[PatchOperationKind.NONE, PatchOperationKind.SetField, PatchOperationKind.UnsetField], table: table.Table) -> typing.Union[None, SetField, UnsetField]: ...
