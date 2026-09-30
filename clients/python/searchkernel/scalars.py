"""Validation for Python values written to fixed-width FlatBuffers scalars."""

INT64_MIN = -(1 << 63)
INT64_MAX = (1 << 63) - 1
UINT32_MAX = (1 << 32) - 1


def check_int64(value: int, label: str = "int64 value") -> int:
    if isinstance(value, bool) or not isinstance(value, int):
        raise TypeError(f"{label} must be an integer")
    if not INT64_MIN <= value <= INT64_MAX:
        raise ValueError(f"{label} is outside int64 range")
    return value


def check_uint32(value: int, label: str) -> int:
    if isinstance(value, bool) or not isinstance(value, int) or not 0 <= value <= UINT32_MAX:
        raise ValueError(f"{label} must be a uint32")
    return value
