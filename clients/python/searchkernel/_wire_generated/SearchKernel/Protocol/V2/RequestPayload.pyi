from __future__ import annotations

import flatbuffers
import numpy as np

import typing
from SearchKernel.V2.AnalyzeRequest import AnalyzeRequest
from SearchKernel.V2.DeleteRequest import DeleteRequest
from SearchKernel.V2.DescribeRequest import DescribeRequest
from SearchKernel.V2.GetRequest import GetRequest
from SearchKernel.V2.InsertRequest import InsertRequest
from SearchKernel.V2.InspectQueryRequest import InspectQueryRequest
from SearchKernel.V2.PatchRequest import PatchRequest
from SearchKernel.V2.SearchRequest import SearchRequest
from SearchKernel.V2.UpsertRequest import UpsertRequest
from flatbuffers import table
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
def RequestPayloadCreator(union_type: typing.Literal[RequestPayload.NONE, RequestPayload.SearchKernel_V2_InsertRequest, RequestPayload.SearchKernel_V2_UpsertRequest, RequestPayload.SearchKernel_V2_PatchRequest, RequestPayload.SearchKernel_V2_DeleteRequest, RequestPayload.SearchKernel_V2_GetRequest, RequestPayload.SearchKernel_V2_SearchRequest, RequestPayload.SearchKernel_V2_AnalyzeRequest, RequestPayload.SearchKernel_V2_DescribeRequest, RequestPayload.SearchKernel_V2_InspectQueryRequest], table: table.Table) -> typing.Union[None, InsertRequest, UpsertRequest, PatchRequest, DeleteRequest, GetRequest, SearchRequest, AnalyzeRequest, DescribeRequest, InspectQueryRequest]: ...

