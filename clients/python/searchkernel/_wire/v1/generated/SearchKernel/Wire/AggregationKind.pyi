from __future__ import annotations

import flatbuffers
import numpy as np

import typing
from SearchKernel.Wire.AvgAggregation import AvgAggregation
from SearchKernel.Wire.CountAggregation import CountAggregation
from SearchKernel.Wire.MaxAggregation import MaxAggregation
from SearchKernel.Wire.MinAggregation import MinAggregation
from SearchKernel.Wire.QuantilesAggregation import QuantilesAggregation
from SearchKernel.Wire.StdDevAggregation import StdDevAggregation
from SearchKernel.Wire.SumAggregation import SumAggregation
from SearchKernel.Wire.SummaryAggregation import SummaryAggregation
from SearchKernel.Wire.XorAggregation import XorAggregation
from flatbuffers import table
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
def AggregationKindCreator(union_type: typing.Literal[AggregationKind.NONE, AggregationKind.CountAggregation, AggregationKind.SumAggregation, AggregationKind.AvgAggregation, AggregationKind.MinAggregation, AggregationKind.MaxAggregation, AggregationKind.XorAggregation, AggregationKind.StdDevAggregation, AggregationKind.QuantilesAggregation, AggregationKind.SummaryAggregation], table: table.Table) -> typing.Union[None, CountAggregation, SumAggregation, AvgAggregation, MinAggregation, MaxAggregation, XorAggregation, StdDevAggregation, QuantilesAggregation, SummaryAggregation]: ...
