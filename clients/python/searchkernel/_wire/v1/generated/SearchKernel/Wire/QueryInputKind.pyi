from __future__ import annotations

import flatbuffers
import numpy as np

import typing
from SearchKernel.Wire.ExpressionQueryInput import ExpressionQueryInput
from SearchKernel.Wire.LiteralQueryInput import LiteralQueryInput
from flatbuffers import table
from typing import cast

uoffset: typing.TypeAlias = flatbuffers.number_types.UOffsetTFlags.py_type

class QueryInputKind(object):
  NONE = cast(int, ...)
  LiteralQueryInput = cast(int, ...)
  ExpressionQueryInput = cast(int, ...)
def QueryInputKindCreator(union_type: typing.Literal[QueryInputKind.NONE, QueryInputKind.LiteralQueryInput, QueryInputKind.ExpressionQueryInput], table: table.Table) -> typing.Union[None, LiteralQueryInput, ExpressionQueryInput]: ...
