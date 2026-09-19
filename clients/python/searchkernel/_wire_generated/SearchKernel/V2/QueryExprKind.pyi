from __future__ import annotations

import flatbuffers
import numpy as np

import typing
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

