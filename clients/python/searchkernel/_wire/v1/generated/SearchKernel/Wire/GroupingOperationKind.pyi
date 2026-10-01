from __future__ import annotations

import flatbuffers
import numpy as np

import typing
from SearchKernel.Wire.GroupingAliasOperation import GroupingAliasOperation
from SearchKernel.Wire.GroupingAllOperation import GroupingAllOperation
from SearchKernel.Wire.GroupingEachOperation import GroupingEachOperation
from SearchKernel.Wire.GroupingFilterOperation import GroupingFilterOperation
from SearchKernel.Wire.GroupingGroupOperation import GroupingGroupOperation
from SearchKernel.Wire.GroupingKeepOperation import GroupingKeepOperation
from SearchKernel.Wire.GroupingMaxOperation import GroupingMaxOperation
from SearchKernel.Wire.GroupingOrderOperation import GroupingOrderOperation
from SearchKernel.Wire.GroupingOutputOperation import GroupingOutputOperation
from SearchKernel.Wire.GroupingPrecisionOperation import GroupingPrecisionOperation
from SearchKernel.Wire.GroupingWhereOperation import GroupingWhereOperation
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
