from __future__ import annotations

import flatbuffers
import numpy as np

import typing
from typing import cast

uoffset: typing.TypeAlias = flatbuffers.number_types.UOffsetTFlags.py_type

class DiagnosticSection(object):
  Query = cast(int, ...)
  Retrieval = cast(int, ...)
  Ranking = cast(int, ...)
  Timing = cast(int, ...)
  MatchedTerms = cast(int, ...)
  Vectors = cast(int, ...)
  Trace = cast(int, ...)
  MatchingProfiler = cast(int, ...)
  FirstPhaseProfiler = cast(int, ...)
  SecondPhaseProfiler = cast(int, ...)
  Blueprint = cast(int, ...)
  IteratorTree = cast(int, ...)
  RawEngine = cast(int, ...)
  Counterfactual = cast(int, ...)
