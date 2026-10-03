from __future__ import annotations

import flatbuffers
import numpy as np

import typing
from SearchKernel.Wire.AnalyzeResult import AnalyzeResult
from SearchKernel.Wire.CreateIndexResult import CreateIndexResult
from SearchKernel.Wire.DeleteIndexResult import DeleteIndexResult
from SearchKernel.Wire.DeleteResult import DeleteResult
from SearchKernel.Wire.DescribeResult import DescribeResult
from SearchKernel.Wire.Error import Error
from SearchKernel.Wire.GetResult import GetResult
from SearchKernel.Wire.InsertResult import InsertResult
from SearchKernel.Wire.InspectQueryResult import InspectQueryResult
from SearchKernel.Wire.ListIndexesResult import ListIndexesResult
from SearchKernel.Wire.OpenIndexResult import OpenIndexResult
from SearchKernel.Wire.PatchResult import PatchResult
from SearchKernel.Wire.SearchResult import SearchResult
from SearchKernel.Wire.UpdateResult import UpdateResult
from SearchKernel.Wire.UpsertResult import UpsertResult
from flatbuffers import table
from typing import cast

uoffset: typing.TypeAlias = flatbuffers.number_types.UOffsetTFlags.py_type

class ResponsePayload(object):
  NONE = cast(int, ...)
  SearchKernel_Wire_InsertResult = cast(int, ...)
  SearchKernel_Wire_UpsertResult = cast(int, ...)
  SearchKernel_Wire_PatchResult = cast(int, ...)
  SearchKernel_Wire_DeleteResult = cast(int, ...)
  SearchKernel_Wire_GetResult = cast(int, ...)
  SearchKernel_Wire_SearchResult = cast(int, ...)
  SearchKernel_Wire_AnalyzeResult = cast(int, ...)
  SearchKernel_Wire_Error = cast(int, ...)
  SearchKernel_Wire_DescribeResult = cast(int, ...)
  SearchKernel_Wire_InspectQueryResult = cast(int, ...)
  SearchKernel_Wire_UpdateResult = cast(int, ...)
  SearchKernel_Wire_CreateIndexResult = cast(int, ...)
  SearchKernel_Wire_OpenIndexResult = cast(int, ...)
  SearchKernel_Wire_DeleteIndexResult = cast(int, ...)
  SearchKernel_Wire_ListIndexesResult = cast(int, ...)
def ResponsePayloadCreator(union_type: typing.Literal[ResponsePayload.NONE, ResponsePayload.SearchKernel_Wire_InsertResult, ResponsePayload.SearchKernel_Wire_UpsertResult, ResponsePayload.SearchKernel_Wire_PatchResult, ResponsePayload.SearchKernel_Wire_DeleteResult, ResponsePayload.SearchKernel_Wire_GetResult, ResponsePayload.SearchKernel_Wire_SearchResult, ResponsePayload.SearchKernel_Wire_AnalyzeResult, ResponsePayload.SearchKernel_Wire_Error, ResponsePayload.SearchKernel_Wire_DescribeResult, ResponsePayload.SearchKernel_Wire_InspectQueryResult, ResponsePayload.SearchKernel_Wire_UpdateResult, ResponsePayload.SearchKernel_Wire_CreateIndexResult, ResponsePayload.SearchKernel_Wire_OpenIndexResult, ResponsePayload.SearchKernel_Wire_DeleteIndexResult, ResponsePayload.SearchKernel_Wire_ListIndexesResult], table: table.Table) -> typing.Union[None, InsertResult, UpsertResult, PatchResult, DeleteResult, GetResult, SearchResult, AnalyzeResult, Error, DescribeResult, InspectQueryResult, UpdateResult, CreateIndexResult, OpenIndexResult, DeleteIndexResult, ListIndexesResult]: ...
