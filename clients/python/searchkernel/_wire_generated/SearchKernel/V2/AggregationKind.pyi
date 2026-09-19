from __future__ import annotations

import flatbuffers
import numpy as np

import typing
from typing import cast

uoffset: typing.TypeAlias = flatbuffers.number_types.UOffsetTFlags.py_type

class AggregationKind(object):
  NONE = cast(int, ...)
  CountAggregation = cast(int, ...)
  SumAggregation = cast(int, ...)
  AvgAggregation = cast(int, ...)
  MinAggregation = cast(int, ...)
  MaxAggregation = cast(int, ...)
  XorAggregation = cast(int, ...)
  StdDevAggregation = cast(int, ...)
  QuantilesAggregation = cast(int, ...)
  SummaryAggregation = cast(int, ...)

