from __future__ import annotations

import flatbuffers
import numpy as np

import typing
from SearchKernel.Wire.AnalyzeRequest import AnalyzeRequest
from SearchKernel.Wire.CreateIndexRequest import CreateIndexRequest
from SearchKernel.Wire.DeleteIndexRequest import DeleteIndexRequest
from SearchKernel.Wire.DeleteRequest import DeleteRequest
from SearchKernel.Wire.DescribeRequest import DescribeRequest
from SearchKernel.Wire.GetRequest import GetRequest
from SearchKernel.Wire.InsertRequest import InsertRequest
from SearchKernel.Wire.InspectQueryRequest import InspectQueryRequest
from SearchKernel.Wire.ListIndexesRequest import ListIndexesRequest
from SearchKernel.Wire.OpenIndexRequest import OpenIndexRequest
from SearchKernel.Wire.PatchRequest import PatchRequest
from SearchKernel.Wire.SearchRequest import SearchRequest
from SearchKernel.Wire.UpdateRequest import UpdateRequest
from SearchKernel.Wire.UpsertRequest import UpsertRequest
from flatbuffers import table
from typing import cast

uoffset: typing.TypeAlias = flatbuffers.number_types.UOffsetTFlags.py_type

class RequestPayload(object):
  NONE = cast(int, ...)
  SearchKernel_Wire_InsertRequest = cast(int, ...)
  SearchKernel_Wire_UpsertRequest = cast(int, ...)
  SearchKernel_Wire_PatchRequest = cast(int, ...)
  SearchKernel_Wire_DeleteRequest = cast(int, ...)
  SearchKernel_Wire_GetRequest = cast(int, ...)
  SearchKernel_Wire_SearchRequest = cast(int, ...)
  SearchKernel_Wire_AnalyzeRequest = cast(int, ...)
  SearchKernel_Wire_DescribeRequest = cast(int, ...)
  SearchKernel_Wire_InspectQueryRequest = cast(int, ...)
  SearchKernel_Wire_UpdateRequest = cast(int, ...)
  SearchKernel_Wire_CreateIndexRequest = cast(int, ...)
  SearchKernel_Wire_OpenIndexRequest = cast(int, ...)
  SearchKernel_Wire_DeleteIndexRequest = cast(int, ...)
  SearchKernel_Wire_ListIndexesRequest = cast(int, ...)
def RequestPayloadCreator(union_type: typing.Literal[RequestPayload.NONE, RequestPayload.SearchKernel_Wire_InsertRequest, RequestPayload.SearchKernel_Wire_UpsertRequest, RequestPayload.SearchKernel_Wire_PatchRequest, RequestPayload.SearchKernel_Wire_DeleteRequest, RequestPayload.SearchKernel_Wire_GetRequest, RequestPayload.SearchKernel_Wire_SearchRequest, RequestPayload.SearchKernel_Wire_AnalyzeRequest, RequestPayload.SearchKernel_Wire_DescribeRequest, RequestPayload.SearchKernel_Wire_InspectQueryRequest, RequestPayload.SearchKernel_Wire_UpdateRequest, RequestPayload.SearchKernel_Wire_CreateIndexRequest, RequestPayload.SearchKernel_Wire_OpenIndexRequest, RequestPayload.SearchKernel_Wire_DeleteIndexRequest, RequestPayload.SearchKernel_Wire_ListIndexesRequest], table: table.Table) -> typing.Union[None, InsertRequest, UpsertRequest, PatchRequest, DeleteRequest, GetRequest, SearchRequest, AnalyzeRequest, DescribeRequest, InspectQueryRequest, UpdateRequest, CreateIndexRequest, OpenIndexRequest, DeleteIndexRequest, ListIndexesRequest]: ...
