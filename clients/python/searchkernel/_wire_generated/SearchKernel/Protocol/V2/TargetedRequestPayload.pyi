from __future__ import annotations

import flatbuffers
import numpy as np

import typing
from SearchKernel.V2.AnalyzeRequest import AnalyzeRequest
from SearchKernel.V2.CreateIndexRequest import CreateIndexRequest
from SearchKernel.V2.DeleteRequest import DeleteRequest
from SearchKernel.V2.DescribeRequest import DescribeRequest
from SearchKernel.V2.GetRequest import GetRequest
from SearchKernel.V2.InsertRequest import InsertRequest
from SearchKernel.V2.InspectQueryRequest import InspectQueryRequest
from SearchKernel.V2.OpenIndexRequest import OpenIndexRequest
from SearchKernel.V2.PatchRequest import PatchRequest
from SearchKernel.V2.SearchRequest import SearchRequest
from SearchKernel.V2.UpdateRequest import UpdateRequest
from SearchKernel.V2.UpsertRequest import UpsertRequest
from flatbuffers import table
from typing import cast

uoffset: typing.TypeAlias = flatbuffers.number_types.UOffsetTFlags.py_type

class TargetedRequestPayload(object):
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
  SearchKernel_V2_UpdateRequest = cast(int, ...)
  SearchKernel_V2_CreateIndexRequest = cast(int, ...)
  SearchKernel_V2_OpenIndexRequest = cast(int, ...)
def TargetedRequestPayloadCreator(union_type: typing.Literal[TargetedRequestPayload.NONE, TargetedRequestPayload.SearchKernel_V2_InsertRequest, TargetedRequestPayload.SearchKernel_V2_UpsertRequest, TargetedRequestPayload.SearchKernel_V2_PatchRequest, TargetedRequestPayload.SearchKernel_V2_DeleteRequest, TargetedRequestPayload.SearchKernel_V2_GetRequest, TargetedRequestPayload.SearchKernel_V2_SearchRequest, TargetedRequestPayload.SearchKernel_V2_AnalyzeRequest, TargetedRequestPayload.SearchKernel_V2_DescribeRequest, TargetedRequestPayload.SearchKernel_V2_InspectQueryRequest, TargetedRequestPayload.SearchKernel_V2_UpdateRequest, TargetedRequestPayload.SearchKernel_V2_CreateIndexRequest, TargetedRequestPayload.SearchKernel_V2_OpenIndexRequest], table: table.Table) -> typing.Union[None, InsertRequest, UpsertRequest, PatchRequest, DeleteRequest, GetRequest, SearchRequest, AnalyzeRequest, DescribeRequest, InspectQueryRequest, UpdateRequest, CreateIndexRequest, OpenIndexRequest]: ...

