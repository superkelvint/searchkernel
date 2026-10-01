from __future__ import annotations

import flatbuffers
import numpy as np

import typing
from SearchKernel.Wire.FileConstantSource import FileConstantSource
from SearchKernel.Wire.LiteralConstantSource import LiteralConstantSource
from flatbuffers import table
from typing import cast

uoffset: typing.TypeAlias = flatbuffers.number_types.UOffsetTFlags.py_type

class ConstantSourceKind(object):
  NONE = cast(int, ...)
  LiteralConstantSource = cast(int, ...)
  FileConstantSource = cast(int, ...)
def ConstantSourceKindCreator(union_type: typing.Literal[ConstantSourceKind.NONE, ConstantSourceKind.LiteralConstantSource, ConstantSourceKind.FileConstantSource], table: table.Table) -> typing.Union[None, LiteralConstantSource, FileConstantSource]: ...
