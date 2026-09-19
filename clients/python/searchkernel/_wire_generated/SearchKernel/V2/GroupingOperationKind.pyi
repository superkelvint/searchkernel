from __future__ import annotations

import flatbuffers
import numpy as np

import typing
from SearchKernel.V2.GroupingAliasOperation import GroupingAliasOperation
from SearchKernel.V2.GroupingAllOperation import GroupingAllOperation
from SearchKernel.V2.GroupingEachOperation import GroupingEachOperation
from SearchKernel.V2.GroupingFilterOperation import GroupingFilterOperation
from SearchKernel.V2.GroupingGroupOperation import GroupingGroupOperation
from SearchKernel.V2.GroupingKeepOperation import GroupingKeepOperation
from SearchKernel.V2.GroupingMaxOperation import GroupingMaxOperation
from SearchKernel.V2.GroupingOrderOperation import GroupingOrderOperation
from SearchKernel.V2.GroupingOutputOperation import GroupingOutputOperation
from SearchKernel.V2.GroupingPrecisionOperation import GroupingPrecisionOperation
from SearchKernel.V2.GroupingWhereOperation import GroupingWhereOperation
from flatbuffers import table
from typing import cast

uoffset: typing.TypeAlias = flatbuffers.number_types.UOffsetTFlags.py_type

class GroupingOperationKind(object):
  NONE = cast(int, ...)
  GroupingAllOperation = cast(int, ...)
  GroupingEachOperation = cast(int, ...)
  GroupingGroupOperation = cast(int, ...)
  GroupingOutputOperation = cast(int, ...)
  GroupingMaxOperation = cast(int, ...)
  GroupingPrecisionOperation = cast(int, ...)
  GroupingOrderOperation = cast(int, ...)
  GroupingAliasOperation = cast(int, ...)
  GroupingFilterOperation = cast(int, ...)
  GroupingKeepOperation = cast(int, ...)
  GroupingWhereOperation = cast(int, ...)
def GroupingOperationKindCreator(union_type: typing.Literal[GroupingOperationKind.NONE, GroupingOperationKind.GroupingAllOperation, GroupingOperationKind.GroupingEachOperation, GroupingOperationKind.GroupingGroupOperation, GroupingOperationKind.GroupingOutputOperation, GroupingOperationKind.GroupingMaxOperation, GroupingOperationKind.GroupingPrecisionOperation, GroupingOperationKind.GroupingOrderOperation, GroupingOperationKind.GroupingAliasOperation, GroupingOperationKind.GroupingFilterOperation, GroupingOperationKind.GroupingKeepOperation, GroupingOperationKind.GroupingWhereOperation], table: table.Table) -> typing.Union[None, GroupingAllOperation, GroupingEachOperation, GroupingGroupOperation, GroupingOutputOperation, GroupingMaxOperation, GroupingPrecisionOperation, GroupingOrderOperation, GroupingAliasOperation, GroupingFilterOperation, GroupingKeepOperation, GroupingWhereOperation]: ...

