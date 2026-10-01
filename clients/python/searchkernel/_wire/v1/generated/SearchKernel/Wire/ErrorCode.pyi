from __future__ import annotations

import flatbuffers
import numpy as np

import typing
from typing import cast

uoffset: typing.TypeAlias = flatbuffers.number_types.UOffsetTFlags.py_type

class ErrorCode(object):
  Unspecified = cast(int, ...)
  InvalidArgument = cast(int, ...)
  NotFound = cast(int, ...)
  AlreadyExists = cast(int, ...)
  FailedPrecondition = cast(int, ...)
  Conflict = cast(int, ...)
  Unsupported = cast(int, ...)
  ResourceExhausted = cast(int, ...)
  Internal = cast(int, ...)
  Unavailable = cast(int, ...)
  DeadlineExceeded = cast(int, ...)
