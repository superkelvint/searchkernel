from __future__ import annotations

import flatbuffers
import numpy as np

import typing
from SearchKernel.V2.MissingAs import MissingAs
from SearchKernel.V2.MissingFirst import MissingFirst
from SearchKernel.V2.MissingLast import MissingLast
from flatbuffers import table
from typing import cast

uoffset: typing.TypeAlias = flatbuffers.number_types.UOffsetTFlags.py_type

class MissingPolicyKind(object):
  NONE = cast(int, ...)
  MissingFirst = cast(int, ...)
  MissingLast = cast(int, ...)
  MissingAs = cast(int, ...)
def MissingPolicyKindCreator(union_type: typing.Literal[MissingPolicyKind.NONE, MissingPolicyKind.MissingFirst, MissingPolicyKind.MissingLast, MissingPolicyKind.MissingAs], table: table.Table) -> typing.Union[None, MissingFirst, MissingLast, MissingAs]: ...

