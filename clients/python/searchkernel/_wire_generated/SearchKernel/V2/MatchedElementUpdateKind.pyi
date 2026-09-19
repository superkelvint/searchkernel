from __future__ import annotations

import flatbuffers
import numpy as np

import typing
from SearchKernel.V2.ArithmeticUpdate import ArithmeticUpdate
from SearchKernel.V2.AssignUpdate import AssignUpdate
from SearchKernel.V2.ClearUpdate import ClearUpdate
from flatbuffers import table
from typing import cast

uoffset: typing.TypeAlias = flatbuffers.number_types.UOffsetTFlags.py_type

class MatchedElementUpdateKind(object):
  NONE = cast(int, ...)
  AssignUpdate = cast(int, ...)
  ClearUpdate = cast(int, ...)
  ArithmeticUpdate = cast(int, ...)
def MatchedElementUpdateKindCreator(union_type: typing.Literal[MatchedElementUpdateKind.NONE, MatchedElementUpdateKind.AssignUpdate, MatchedElementUpdateKind.ClearUpdate, MatchedElementUpdateKind.ArithmeticUpdate], table: table.Table) -> typing.Union[None, AssignUpdate, ClearUpdate, ArithmeticUpdate]: ...

