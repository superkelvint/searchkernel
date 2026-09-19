from __future__ import annotations

import flatbuffers
import numpy as np

import typing
from SearchKernel.V2.QueryAlternatives import QueryAlternativesT
from SearchKernel.V2.QueryAnd import QueryAndT
from SearchKernel.V2.QueryComparison import QueryComparisonT
from SearchKernel.V2.QueryContains import QueryContainsT
from SearchKernel.V2.QueryDotProduct import QueryDotProductT
from SearchKernel.V2.QueryEquiv import QueryEquivT
from SearchKernel.V2.QueryExprKind import QueryExprKind
from SearchKernel.V2.QueryFalse import QueryFalseT
from SearchKernel.V2.QueryFuzzy import QueryFuzzyT
from SearchKernel.V2.QueryGeoBoundingBox import QueryGeoBoundingBoxT
from SearchKernel.V2.QueryGeoLocation import QueryGeoLocationT
from SearchKernel.V2.QueryIn import QueryInT
from SearchKernel.V2.QueryLabeled import QueryLabeledT
from SearchKernel.V2.QueryMatches import QueryMatchesT
from SearchKernel.V2.QueryNear import QueryNearT
from SearchKernel.V2.QueryNearestNeighbor import QueryNearestNeighborT
from SearchKernel.V2.QueryNonEmpty import QueryNonEmptyT
from SearchKernel.V2.QueryNot import QueryNotT
from SearchKernel.V2.QueryONear import QueryONearT
from SearchKernel.V2.QueryOr import QueryOrT
from SearchKernel.V2.QueryPhrase import QueryPhraseT
from SearchKernel.V2.QueryPredicate import QueryPredicateT
from SearchKernel.V2.QueryRange import QueryRangeT
from SearchKernel.V2.QueryRank import QueryRankT
from SearchKernel.V2.QuerySameElement import QuerySameElementT
from SearchKernel.V2.QueryText import QueryTextT
from SearchKernel.V2.QueryTrue import QueryTrueT
from SearchKernel.V2.QueryUserInput import QueryUserInputT
from SearchKernel.V2.QueryUserQuery import QueryUserQueryT
from SearchKernel.V2.QueryWand import QueryWandT
from SearchKernel.V2.QueryWeakAnd import QueryWeakAndT
from SearchKernel.V2.QueryWeightedSet import QueryWeightedSetT
from SearchKernel.V2.TermQuery import TermQueryT
from SearchKernel.V2.ValueEntry import ValueEntry, ValueEntryT
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

