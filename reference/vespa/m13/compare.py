#!/usr/bin/env python3
import json
import math
import sys

expected = json.load(open(sys.argv[1], encoding="utf-8"))
actual = json.load(sys.stdin)


def equal(left, right):
    if isinstance(left, bool) or isinstance(right, bool):
        return left == right
    if isinstance(left, (int, float)) and isinstance(right, (int, float)):
        return math.isclose(float(left), float(right), rel_tol=1e-12, abs_tol=1e-11)
    if isinstance(left, dict) and isinstance(right, dict):
        return left.keys() == right.keys() and all(
            equal(left[key], right[key]) for key in left
        )
    if isinstance(left, list) and isinstance(right, list):
        return len(left) == len(right) and all(
            equal(item_left, item_right) for item_left, item_right in zip(left, right)
        )
    return left == right


if not equal(actual, expected):
    raise SystemExit(f"M13 output mismatch\nexpected={expected!r}\nactual={actual!r}")
