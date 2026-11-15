from __future__ import annotations

import flatbuffers
import numpy as np

import typing
from typing import cast

uoffset: typing.TypeAlias = flatbuffers.number_types.UOffsetTFlags.py_type

class DiagnosticExecutionKind(object):
  Original = cast(int, ...)
  DiagnosticRerun = cast(int, ...)
  Experiment = cast(int, ...)
  Counterfactual = cast(int, ...)
