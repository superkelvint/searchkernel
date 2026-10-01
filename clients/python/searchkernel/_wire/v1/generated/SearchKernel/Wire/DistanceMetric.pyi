from __future__ import annotations

import flatbuffers
import numpy as np

import typing
from typing import cast

uoffset: typing.TypeAlias = flatbuffers.number_types.UOffsetTFlags.py_type

class DistanceMetric(object):
  Unspecified = cast(int, ...)
  Euclidean = cast(int, ...)
  Angular = cast(int, ...)
  DotProduct = cast(int, ...)
  PrenormalizedAngular = cast(int, ...)
  GeoDegrees = cast(int, ...)
  Hamming = cast(int, ...)
