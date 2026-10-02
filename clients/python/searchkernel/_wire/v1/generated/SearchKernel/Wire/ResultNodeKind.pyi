from __future__ import annotations

import flatbuffers
import numpy as np

import typing
from SearchKernel.Wire.Hit import Hit
from SearchKernel.Wire.ResultGroup import ResultGroup
from flatbuffers import table
from typing import cast

uoffset: typing.TypeAlias = flatbuffers.number_types.UOffsetTFlags.py_type

class ResultNodeKind(object):
  NONE = cast(int, ...)
  Hit = cast(int, ...)
  ResultGroup = cast(int, ...)
def ResultNodeKindCreator(union_type: typing.Literal[ResultNodeKind.NONE, ResultNodeKind.Hit, ResultNodeKind.ResultGroup], table: table.Table) -> typing.Union[None, Hit, ResultGroup]: ...
