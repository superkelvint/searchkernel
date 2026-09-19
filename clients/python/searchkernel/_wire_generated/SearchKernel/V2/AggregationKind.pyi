from __future__ import annotations

import flatbuffers
import numpy as np

import typing
from SearchKernel.V2.AvgAggregation import AvgAggregation
from SearchKernel.V2.CountAggregation import CountAggregation
from SearchKernel.V2.MaxAggregation import MaxAggregation
from SearchKernel.V2.MinAggregation import MinAggregation
from SearchKernel.V2.QuantilesAggregation import QuantilesAggregation
from SearchKernel.V2.StdDevAggregation import StdDevAggregation
from SearchKernel.V2.SumAggregation import SumAggregation
from SearchKernel.V2.SummaryAggregation import SummaryAggregation
from SearchKernel.V2.XorAggregation import XorAggregation
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

