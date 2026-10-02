from __future__ import annotations

import flatbuffers
import numpy as np

import typing
from SearchKernel.Wire.QueryAlternatives import QueryAlternatives
from SearchKernel.Wire.QueryAnd import QueryAnd
from SearchKernel.Wire.QueryComparison import QueryComparison
from SearchKernel.Wire.QueryContains import QueryContains
from SearchKernel.Wire.QueryDotProduct import QueryDotProduct
from SearchKernel.Wire.QueryEquiv import QueryEquiv
from SearchKernel.Wire.QueryFalse import QueryFalse
from SearchKernel.Wire.QueryFuzzy import QueryFuzzy
from SearchKernel.Wire.QueryGeoBoundingBox import QueryGeoBoundingBox
from SearchKernel.Wire.QueryGeoLocation import QueryGeoLocation
from SearchKernel.Wire.QueryIn import QueryIn
from SearchKernel.Wire.QueryLabeled import QueryLabeled
from SearchKernel.Wire.QueryMatches import QueryMatches
from SearchKernel.Wire.QueryNear import QueryNear
from SearchKernel.Wire.QueryNearestNeighbor import QueryNearestNeighbor
from SearchKernel.Wire.QueryNonEmpty import QueryNonEmpty
from SearchKernel.Wire.QueryNot import QueryNot
from SearchKernel.Wire.QueryONear import QueryONear
from SearchKernel.Wire.QueryOr import QueryOr
from SearchKernel.Wire.QueryPhrase import QueryPhrase
from SearchKernel.Wire.QueryPredicate import QueryPredicate
from SearchKernel.Wire.QueryRange import QueryRange
from SearchKernel.Wire.QueryRank import QueryRank
from SearchKernel.Wire.QuerySameElement import QuerySameElement
from SearchKernel.Wire.QueryText import QueryText
from SearchKernel.Wire.QueryTrue import QueryTrue
from SearchKernel.Wire.QueryUserInput import QueryUserInput
from SearchKernel.Wire.QueryUserQuery import QueryUserQuery
from SearchKernel.Wire.QueryWand import QueryWand
from SearchKernel.Wire.QueryWeakAnd import QueryWeakAnd
from SearchKernel.Wire.QueryWeightedSet import QueryWeightedSet
from SearchKernel.Wire.TermQuery import TermQuery
from flatbuffers import table
from typing import cast

uoffset: typing.TypeAlias = flatbuffers.number_types.UOffsetTFlags.py_type

class QueryExprKind(object):
  NONE = cast(int, ...)
  TermQuery = cast(int, ...)
  QueryContains = cast(int, ...)
  QueryAnd = cast(int, ...)
  QueryOr = cast(int, ...)
  QueryNot = cast(int, ...)
  QueryTrue = cast(int, ...)
  QueryFalse = cast(int, ...)
  QueryComparison = cast(int, ...)
  QueryRange = cast(int, ...)
  QueryIn = cast(int, ...)
  QueryPhrase = cast(int, ...)
  QueryNear = cast(int, ...)
  QueryONear = cast(int, ...)
  QuerySameElement = cast(int, ...)
  QueryEquiv = cast(int, ...)
  QueryAlternatives = cast(int, ...)
  QueryFuzzy = cast(int, ...)
  QueryMatches = cast(int, ...)
  QueryText = cast(int, ...)
  QueryUserInput = cast(int, ...)
  QueryUserQuery = cast(int, ...)
  QueryDotProduct = cast(int, ...)
  QueryWeightedSet = cast(int, ...)
  QueryWand = cast(int, ...)
  QueryWeakAnd = cast(int, ...)
  QueryRank = cast(int, ...)
  QueryLabeled = cast(int, ...)
  QueryNonEmpty = cast(int, ...)
  QueryGeoLocation = cast(int, ...)
  QueryGeoBoundingBox = cast(int, ...)
  QueryNearestNeighbor = cast(int, ...)
  QueryPredicate = cast(int, ...)
def QueryExprKindCreator(union_type: typing.Literal[QueryExprKind.NONE, QueryExprKind.TermQuery, QueryExprKind.QueryContains, QueryExprKind.QueryAnd, QueryExprKind.QueryOr, QueryExprKind.QueryNot, QueryExprKind.QueryTrue, QueryExprKind.QueryFalse, QueryExprKind.QueryComparison, QueryExprKind.QueryRange, QueryExprKind.QueryIn, QueryExprKind.QueryPhrase, QueryExprKind.QueryNear, QueryExprKind.QueryONear, QueryExprKind.QuerySameElement, QueryExprKind.QueryEquiv, QueryExprKind.QueryAlternatives, QueryExprKind.QueryFuzzy, QueryExprKind.QueryMatches, QueryExprKind.QueryText, QueryExprKind.QueryUserInput, QueryExprKind.QueryUserQuery, QueryExprKind.QueryDotProduct, QueryExprKind.QueryWeightedSet, QueryExprKind.QueryWand, QueryExprKind.QueryWeakAnd, QueryExprKind.QueryRank, QueryExprKind.QueryLabeled, QueryExprKind.QueryNonEmpty, QueryExprKind.QueryGeoLocation, QueryExprKind.QueryGeoBoundingBox, QueryExprKind.QueryNearestNeighbor, QueryExprKind.QueryPredicate], table: table.Table) -> typing.Union[None, TermQuery, QueryContains, QueryAnd, QueryOr, QueryNot, QueryTrue, QueryFalse, QueryComparison, QueryRange, QueryIn, QueryPhrase, QueryNear, QueryONear, QuerySameElement, QueryEquiv, QueryAlternatives, QueryFuzzy, QueryMatches, QueryText, QueryUserInput, QueryUserQuery, QueryDotProduct, QueryWeightedSet, QueryWand, QueryWeakAnd, QueryRank, QueryLabeled, QueryNonEmpty, QueryGeoLocation, QueryGeoBoundingBox, QueryNearestNeighbor, QueryPredicate]: ...
