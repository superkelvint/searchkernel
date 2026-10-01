from __future__ import annotations

import flatbuffers
import numpy as np

import typing
from SearchKernel.Wire.AnalyzeFieldTarget import AnalyzeFieldTarget
from SearchKernel.Wire.Analyzer import Analyzer
from flatbuffers import table
from typing import cast

uoffset: typing.TypeAlias = flatbuffers.number_types.UOffsetTFlags.py_type

class AnalyzeTarget(object):
  NONE = cast(int, ...)
  Analyzer = cast(int, ...)
  AnalyzeFieldTarget = cast(int, ...)
def AnalyzeTargetCreator(union_type: typing.Literal[AnalyzeTarget.NONE, AnalyzeTarget.Analyzer, AnalyzeTarget.AnalyzeFieldTarget], table: table.Table) -> typing.Union[None, Analyzer, AnalyzeFieldTarget]: ...
