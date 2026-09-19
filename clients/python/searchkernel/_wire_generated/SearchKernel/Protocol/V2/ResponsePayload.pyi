from __future__ import annotations

import flatbuffers
import numpy as np

import typing
from typing import cast

uoffset: typing.TypeAlias = flatbuffers.number_types.UOffsetTFlags.py_type

class ResponsePayload(object):
  NONE = cast(int, ...)
  SearchKernel_V2_InsertResult = cast(int, ...)
  SearchKernel_V2_UpsertResult = cast(int, ...)
  SearchKernel_V2_PatchResult = cast(int, ...)
  SearchKernel_V2_DeleteResult = cast(int, ...)
  SearchKernel_V2_GetResult = cast(int, ...)
  SearchKernel_V2_SearchResult = cast(int, ...)
  SearchKernel_V2_AnalyzeResult = cast(int, ...)
  SearchKernel_V2_Error = cast(int, ...)
  SearchKernel_V2_DescribeResult = cast(int, ...)
  SearchKernel_V2_InspectQueryResult = cast(int, ...)

