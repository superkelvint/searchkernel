from __future__ import annotations

import flatbuffers
import numpy as np

import typing
from typing import cast

uoffset: typing.TypeAlias = flatbuffers.number_types.UOffsetTFlags.py_type

class RequestPayload(object):
  NONE = cast(int, ...)
  SearchKernel_V2_InsertRequest = cast(int, ...)
  SearchKernel_V2_UpsertRequest = cast(int, ...)
  SearchKernel_V2_PatchRequest = cast(int, ...)
  SearchKernel_V2_DeleteRequest = cast(int, ...)
  SearchKernel_V2_GetRequest = cast(int, ...)
  SearchKernel_V2_SearchRequest = cast(int, ...)
  SearchKernel_V2_AnalyzeRequest = cast(int, ...)
  SearchKernel_V2_DescribeRequest = cast(int, ...)
  SearchKernel_V2_InspectQueryRequest = cast(int, ...)

