from __future__ import annotations

import flatbuffers
import numpy as np

import typing
from SearchKernel.V2.AnalyzeResult import AnalyzeResult
from SearchKernel.V2.CreateIndexResult import CreateIndexResult
from SearchKernel.V2.DeleteIndexResult import DeleteIndexResult
from SearchKernel.V2.DeleteResult import DeleteResult
from SearchKernel.V2.DescribeResult import DescribeResult
from SearchKernel.V2.Error import Error
from SearchKernel.V2.GetResult import GetResult
from SearchKernel.V2.InsertResult import InsertResult
from SearchKernel.V2.InspectQueryResult import InspectQueryResult
from SearchKernel.V2.OpenIndexResult import OpenIndexResult
from SearchKernel.V2.PatchResult import PatchResult
from SearchKernel.V2.SearchResult import SearchResult
from SearchKernel.V2.UpdateResult import UpdateResult
from SearchKernel.V2.UpsertResult import UpsertResult
from flatbuffers import table
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
  SearchKernel_V2_UpdateResult = cast(int, ...)
  SearchKernel_V2_CreateIndexResult = cast(int, ...)
  SearchKernel_V2_OpenIndexResult = cast(int, ...)
  SearchKernel_V2_DeleteIndexResult = cast(int, ...)
def ResponsePayloadCreator(union_type: typing.Literal[ResponsePayload.NONE, ResponsePayload.SearchKernel_V2_InsertResult, ResponsePayload.SearchKernel_V2_UpsertResult, ResponsePayload.SearchKernel_V2_PatchResult, ResponsePayload.SearchKernel_V2_DeleteResult, ResponsePayload.SearchKernel_V2_GetResult, ResponsePayload.SearchKernel_V2_SearchResult, ResponsePayload.SearchKernel_V2_AnalyzeResult, ResponsePayload.SearchKernel_V2_Error, ResponsePayload.SearchKernel_V2_DescribeResult, ResponsePayload.SearchKernel_V2_InspectQueryResult, ResponsePayload.SearchKernel_V2_UpdateResult, ResponsePayload.SearchKernel_V2_CreateIndexResult, ResponsePayload.SearchKernel_V2_OpenIndexResult, ResponsePayload.SearchKernel_V2_DeleteIndexResult], table: table.Table) -> typing.Union[None, InsertResult, UpsertResult, PatchResult, DeleteResult, GetResult, SearchResult, AnalyzeResult, Error, DescribeResult, InspectQueryResult, UpdateResult, CreateIndexResult, OpenIndexResult, DeleteIndexResult]: ...

