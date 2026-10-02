from __future__ import annotations

import flatbuffers
import numpy as np

import typing
from SearchKernel.Wire.QueryAlternatives import QueryAlternativesT
from SearchKernel.Wire.QueryAnd import QueryAndT
from SearchKernel.Wire.QueryComparison import QueryComparisonT
from SearchKernel.Wire.QueryContains import QueryContainsT
from SearchKernel.Wire.QueryDotProduct import QueryDotProductT
from SearchKernel.Wire.QueryEquiv import QueryEquivT
from SearchKernel.Wire.QueryExprKind import QueryExprKind
from SearchKernel.Wire.QueryFalse import QueryFalseT
from SearchKernel.Wire.QueryFuzzy import QueryFuzzyT
from SearchKernel.Wire.QueryGeoBoundingBox import QueryGeoBoundingBoxT
from SearchKernel.Wire.QueryGeoLocation import QueryGeoLocationT
from SearchKernel.Wire.QueryIn import QueryInT
from SearchKernel.Wire.QueryLabeled import QueryLabeledT
from SearchKernel.Wire.QueryMatches import QueryMatchesT
from SearchKernel.Wire.QueryNear import QueryNearT
from SearchKernel.Wire.QueryNearestNeighbor import QueryNearestNeighborT
from SearchKernel.Wire.QueryNonEmpty import QueryNonEmptyT
from SearchKernel.Wire.QueryNot import QueryNotT
from SearchKernel.Wire.QueryONear import QueryONearT
from SearchKernel.Wire.QueryOr import QueryOrT
from SearchKernel.Wire.QueryPhrase import QueryPhraseT
from SearchKernel.Wire.QueryPredicate import QueryPredicateT
from SearchKernel.Wire.QueryRange import QueryRangeT
from SearchKernel.Wire.QueryRank import QueryRankT
from SearchKernel.Wire.QuerySameElement import QuerySameElementT
from SearchKernel.Wire.QueryText import QueryTextT
from SearchKernel.Wire.QueryTrue import QueryTrueT
from SearchKernel.Wire.QueryUserInput import QueryUserInputT
from SearchKernel.Wire.QueryUserQuery import QueryUserQueryT
from SearchKernel.Wire.QueryWand import QueryWandT
from SearchKernel.Wire.QueryWeakAnd import QueryWeakAndT
from SearchKernel.Wire.QueryWeightedSet import QueryWeightedSetT
from SearchKernel.Wire.TermQuery import TermQueryT
from SearchKernel.Wire.ValueEntry import ValueEntry, ValueEntryT
from flatbuffers import table

uoffset: typing.TypeAlias = flatbuffers.number_types.UOffsetTFlags.py_type

class QueryExpr(object):
  @classmethod
  def GetRootAs(cls, buf: bytes, offset: int) -> QueryExpr: ...
  @classmethod
  def GetRootAsQueryExpr(cls, buf: bytes, offset: int) -> QueryExpr: ...
  def Init(self, buf: bytes, pos: int) -> None: ...
  def ExprType(self) -> typing.Literal[QueryExprKind.NONE, QueryExprKind.TermQuery, QueryExprKind.QueryContains, QueryExprKind.QueryAnd, QueryExprKind.QueryOr, QueryExprKind.QueryNot, QueryExprKind.QueryTrue, QueryExprKind.QueryFalse, QueryExprKind.QueryComparison, QueryExprKind.QueryRange, QueryExprKind.QueryIn, QueryExprKind.QueryPhrase, QueryExprKind.QueryNear, QueryExprKind.QueryONear, QueryExprKind.QuerySameElement, QueryExprKind.QueryEquiv, QueryExprKind.QueryAlternatives, QueryExprKind.QueryFuzzy, QueryExprKind.QueryMatches, QueryExprKind.QueryText, QueryExprKind.QueryUserInput, QueryExprKind.QueryUserQuery, QueryExprKind.QueryDotProduct, QueryExprKind.QueryWeightedSet, QueryExprKind.QueryWand, QueryExprKind.QueryWeakAnd, QueryExprKind.QueryRank, QueryExprKind.QueryLabeled, QueryExprKind.QueryNonEmpty, QueryExprKind.QueryGeoLocation, QueryExprKind.QueryGeoBoundingBox, QueryExprKind.QueryNearestNeighbor, QueryExprKind.QueryPredicate]: ...
  def Expr(self) -> table.Table | None: ...
  def Annotations(self, i: int) -> ValueEntry | None: ...
  def AnnotationsLength(self) -> int: ...
  def AnnotationsIsNone(self) -> bool: ...
class QueryExprT(object):
  exprType: typing.Literal[QueryExprKind.NONE, QueryExprKind.TermQuery, QueryExprKind.QueryContains, QueryExprKind.QueryAnd, QueryExprKind.QueryOr, QueryExprKind.QueryNot, QueryExprKind.QueryTrue, QueryExprKind.QueryFalse, QueryExprKind.QueryComparison, QueryExprKind.QueryRange, QueryExprKind.QueryIn, QueryExprKind.QueryPhrase, QueryExprKind.QueryNear, QueryExprKind.QueryONear, QueryExprKind.QuerySameElement, QueryExprKind.QueryEquiv, QueryExprKind.QueryAlternatives, QueryExprKind.QueryFuzzy, QueryExprKind.QueryMatches, QueryExprKind.QueryText, QueryExprKind.QueryUserInput, QueryExprKind.QueryUserQuery, QueryExprKind.QueryDotProduct, QueryExprKind.QueryWeightedSet, QueryExprKind.QueryWand, QueryExprKind.QueryWeakAnd, QueryExprKind.QueryRank, QueryExprKind.QueryLabeled, QueryExprKind.QueryNonEmpty, QueryExprKind.QueryGeoLocation, QueryExprKind.QueryGeoBoundingBox, QueryExprKind.QueryNearestNeighbor, QueryExprKind.QueryPredicate]
  expr: typing.Union[None, TermQueryT, QueryContainsT, QueryAndT, QueryOrT, QueryNotT, QueryTrueT, QueryFalseT, QueryComparisonT, QueryRangeT, QueryInT, QueryPhraseT, QueryNearT, QueryONearT, QuerySameElementT, QueryEquivT, QueryAlternativesT, QueryFuzzyT, QueryMatchesT, QueryTextT, QueryUserInputT, QueryUserQueryT, QueryDotProductT, QueryWeightedSetT, QueryWandT, QueryWeakAndT, QueryRankT, QueryLabeledT, QueryNonEmptyT, QueryGeoLocationT, QueryGeoBoundingBoxT, QueryNearestNeighborT, QueryPredicateT]
  annotations: typing.List[ValueEntryT]
  def __init__(
    self,
    exprType: typing.Literal[QueryExprKind.NONE, QueryExprKind.TermQuery, QueryExprKind.QueryContains, QueryExprKind.QueryAnd, QueryExprKind.QueryOr, QueryExprKind.QueryNot, QueryExprKind.QueryTrue, QueryExprKind.QueryFalse, QueryExprKind.QueryComparison, QueryExprKind.QueryRange, QueryExprKind.QueryIn, QueryExprKind.QueryPhrase, QueryExprKind.QueryNear, QueryExprKind.QueryONear, QueryExprKind.QuerySameElement, QueryExprKind.QueryEquiv, QueryExprKind.QueryAlternatives, QueryExprKind.QueryFuzzy, QueryExprKind.QueryMatches, QueryExprKind.QueryText, QueryExprKind.QueryUserInput, QueryExprKind.QueryUserQuery, QueryExprKind.QueryDotProduct, QueryExprKind.QueryWeightedSet, QueryExprKind.QueryWand, QueryExprKind.QueryWeakAnd, QueryExprKind.QueryRank, QueryExprKind.QueryLabeled, QueryExprKind.QueryNonEmpty, QueryExprKind.QueryGeoLocation, QueryExprKind.QueryGeoBoundingBox, QueryExprKind.QueryNearestNeighbor, QueryExprKind.QueryPredicate] = ...,
    expr: typing.Union[None, TermQueryT, QueryContainsT, QueryAndT, QueryOrT, QueryNotT, QueryTrueT, QueryFalseT, QueryComparisonT, QueryRangeT, QueryInT, QueryPhraseT, QueryNearT, QueryONearT, QuerySameElementT, QueryEquivT, QueryAlternativesT, QueryFuzzyT, QueryMatchesT, QueryTextT, QueryUserInputT, QueryUserQueryT, QueryDotProductT, QueryWeightedSetT, QueryWandT, QueryWeakAndT, QueryRankT, QueryLabeledT, QueryNonEmptyT, QueryGeoLocationT, QueryGeoBoundingBoxT, QueryNearestNeighborT, QueryPredicateT] = ...,
    annotations: typing.List['ValueEntryT'] | None = ...,
  ) -> None: ...
  @classmethod
  def InitFromBuf(cls, buf: bytes, pos: int) -> QueryExprT: ...
  @classmethod
  def InitFromPackedBuf(cls, buf: bytes, pos: int = 0) -> QueryExprT: ...
  @classmethod
  def InitFromObj(cls, queryExpr: QueryExpr) -> QueryExprT: ...
  def _UnPack(self, queryExpr: QueryExpr) -> None: ...
  def Pack(self, builder: flatbuffers.Builder) -> None: ...
def QueryExprStart(builder: flatbuffers.Builder) -> None: ...
def Start(builder: flatbuffers.Builder) -> None: ...
def QueryExprAddExprType(builder: flatbuffers.Builder, exprType: typing.Literal[QueryExprKind.NONE, QueryExprKind.TermQuery, QueryExprKind.QueryContains, QueryExprKind.QueryAnd, QueryExprKind.QueryOr, QueryExprKind.QueryNot, QueryExprKind.QueryTrue, QueryExprKind.QueryFalse, QueryExprKind.QueryComparison, QueryExprKind.QueryRange, QueryExprKind.QueryIn, QueryExprKind.QueryPhrase, QueryExprKind.QueryNear, QueryExprKind.QueryONear, QueryExprKind.QuerySameElement, QueryExprKind.QueryEquiv, QueryExprKind.QueryAlternatives, QueryExprKind.QueryFuzzy, QueryExprKind.QueryMatches, QueryExprKind.QueryText, QueryExprKind.QueryUserInput, QueryExprKind.QueryUserQuery, QueryExprKind.QueryDotProduct, QueryExprKind.QueryWeightedSet, QueryExprKind.QueryWand, QueryExprKind.QueryWeakAnd, QueryExprKind.QueryRank, QueryExprKind.QueryLabeled, QueryExprKind.QueryNonEmpty, QueryExprKind.QueryGeoLocation, QueryExprKind.QueryGeoBoundingBox, QueryExprKind.QueryNearestNeighbor, QueryExprKind.QueryPredicate]) -> None: ...
def QueryExprAddExpr(builder: flatbuffers.Builder, expr: uoffset) -> None: ...
def QueryExprAddAnnotations(builder: flatbuffers.Builder, annotations: uoffset) -> None: ...
def QueryExprStartAnnotationsVector(builder: flatbuffers.Builder, num_elems: int) -> uoffset: ...
def StartAnnotationsVector(builder: flatbuffers.Builder, num_elems: int) -> uoffset: ...
def QueryExprEnd(builder: flatbuffers.Builder) -> uoffset: ...
def End(builder: flatbuffers.Builder) -> uoffset: ...
