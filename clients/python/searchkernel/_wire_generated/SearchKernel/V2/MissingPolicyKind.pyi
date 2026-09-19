from __future__ import annotations

import flatbuffers
import numpy as np

import typing
from typing import cast

uoffset: typing.TypeAlias = flatbuffers.number_types.UOffsetTFlags.py_type

class MissingPolicyKind(object):
  NONE = cast(int, ...)
  MissingFirst = cast(int, ...)
  MissingLast = cast(int, ...)
  MissingAs = cast(int, ...)

