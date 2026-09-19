from __future__ import annotations

import flatbuffers
import numpy as np

import typing
from typing import cast

uoffset: typing.TypeAlias = flatbuffers.number_types.UOffsetTFlags.py_type

class QuerySourceKind(object):
  NONE = cast(int, ...)
  SearchKernelTextQuery = cast(int, ...)
  TypedQuery = cast(int, ...)
  YqlQuery = cast(int, ...)
  MatchAllQuery = cast(int, ...)

