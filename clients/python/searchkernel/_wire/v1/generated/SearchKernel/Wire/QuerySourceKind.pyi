from __future__ import annotations

import flatbuffers
import numpy as np

import typing
from SearchKernel.Wire.MatchAllQuery import MatchAllQuery
from SearchKernel.Wire.SearchKernelTextQuery import SearchKernelTextQuery
from SearchKernel.Wire.TypedQuery import TypedQuery
from SearchKernel.Wire.YqlQuery import YqlQuery
from flatbuffers import table
from typing import cast

uoffset: typing.TypeAlias = flatbuffers.number_types.UOffsetTFlags.py_type

class QuerySourceKind(object):
  NONE = cast(int, ...)
  SearchKernelTextQuery = cast(int, ...)
  TypedQuery = cast(int, ...)
  YqlQuery = cast(int, ...)
  MatchAllQuery = cast(int, ...)
def QuerySourceKindCreator(union_type: typing.Literal[QuerySourceKind.NONE, QuerySourceKind.SearchKernelTextQuery, QuerySourceKind.TypedQuery, QuerySourceKind.YqlQuery, QuerySourceKind.MatchAllQuery], table: table.Table) -> typing.Union[None, SearchKernelTextQuery, TypedQuery, YqlQuery, MatchAllQuery]: ...
