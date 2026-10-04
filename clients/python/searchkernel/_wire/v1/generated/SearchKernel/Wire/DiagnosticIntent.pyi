from __future__ import annotations

import flatbuffers
import numpy as np

import typing
from typing import cast

uoffset: typing.TypeAlias = flatbuffers.number_types.UOffsetTFlags.py_type

class DiagnosticIntent(object):
  Unspecified = cast(int, ...)
  ExplainHit = cast(int, ...)
  ZeroResults = cast(int, ...)
  MissingDocument = cast(int, ...)
  RankingComparison = cast(int, ...)
  Overmatching = cast(int, ...)
  QueryInterpretation = cast(int, ...)
  VectorRetrieval = cast(int, ...)
  Performance = cast(int, ...)
  DeepEngine = cast(int, ...)
  Experiment = cast(int, ...)
