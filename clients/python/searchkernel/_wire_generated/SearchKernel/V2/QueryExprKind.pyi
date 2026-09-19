from __future__ import annotations

import flatbuffers
import numpy as np

import typing
from SearchKernel.V2.QueryAlternatives import QueryAlternatives
from SearchKernel.V2.QueryAnd import QueryAnd
from SearchKernel.V2.QueryComparison import QueryComparison
from SearchKernel.V2.QueryContains import QueryContains
from SearchKernel.V2.QueryDotProduct import QueryDotProduct
from SearchKernel.V2.QueryEquiv import QueryEquiv
from SearchKernel.V2.QueryFalse import QueryFalse
from SearchKernel.V2.QueryFuzzy import QueryFuzzy
from SearchKernel.V2.QueryGeoBoundingBox import QueryGeoBoundingBox
from SearchKernel.V2.QueryGeoLocation import QueryGeoLocation
from SearchKernel.V2.QueryIn import QueryIn
from SearchKernel.V2.QueryLabeled import QueryLabeled
from SearchKernel.V2.QueryMatches import QueryMatches
from SearchKernel.V2.QueryNear import QueryNear
from SearchKernel.V2.QueryNearestNeighbor import QueryNearestNeighbor
from SearchKernel.V2.QueryNonEmpty import QueryNonEmpty
from SearchKernel.V2.QueryNot import QueryNot
from SearchKernel.V2.QueryONear import QueryONear
from SearchKernel.V2.QueryOr import QueryOr
from SearchKernel.V2.QueryPhrase import QueryPhrase
from SearchKernel.V2.QueryPredicate import QueryPredicate
from SearchKernel.V2.QueryRange import QueryRange
from SearchKernel.V2.QueryRank import QueryRank
from SearchKernel.V2.QuerySameElement import QuerySameElement
from SearchKernel.V2.QueryText import QueryText
from SearchKernel.V2.QueryTrue import QueryTrue
from SearchKernel.V2.QueryUserInput import QueryUserInput
from SearchKernel.V2.QueryUserQuery import QueryUserQuery
from SearchKernel.V2.QueryWand import QueryWand
from SearchKernel.V2.QueryWeakAnd import QueryWeakAnd
from SearchKernel.V2.QueryWeightedSet import QueryWeightedSet
from SearchKernel.V2.TermQuery import TermQuery
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

