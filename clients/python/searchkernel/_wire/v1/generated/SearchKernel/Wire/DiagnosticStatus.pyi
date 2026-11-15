from __future__ import annotations

import flatbuffers
import numpy as np

import typing
from typing import cast

uoffset: typing.TypeAlias = flatbuffers.number_types.UOffsetTFlags.py_type

class DiagnosticStatus(object):
  Available = cast(int, ...)
  Unsupported = cast(int, ...)
  TimedOut = cast(int, ...)
  Truncated = cast(int, ...)
  UnavailableForQuery = cast(int, ...)
  Failed = cast(int, ...)
