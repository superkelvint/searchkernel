#!/usr/bin/env python3
import json
import math
import sys
from pathlib import Path

ABS_TOL = 1e-9
REL_TOL = 1e-7


def load(path):
    return [json.loads(line) for line in Path(path).read_text(encoding="utf-8").splitlines() if line.strip()]


def close(a, b):
    return math.isclose(float(a), float(b), rel_tol=REL_TOL, abs_tol=ABS_TOL)


def main():
    if len(sys.argv) != 3:
        raise SystemExit(f"usage: {sys.argv[0]} EXPECTED ACTUAL")
    exp, act = load(sys.argv[1]), load(sys.argv[2])
    if len(exp) != len(act):
        raise SystemExit(f"checkpoint count mismatch: expected {len(exp)}, got {len(act)}")
    for i, (e, a) in enumerate(zip(exp, act), 1):
        if e.get("id") != a.get("id"):
            raise SystemExit(f"checkpoint {i} id mismatch: {e.get('id')!r} != {a.get('id')!r}")
        eh, ah = e.get("hits"), a.get("hits")
        if not isinstance(eh, list) or not isinstance(ah, list):
            raise SystemExit(f"{e['id']}: hits must be arrays")
        if [h.get("id") for h in eh] != [h.get("id") for h in ah]:
            raise SystemExit(
                f"{e['id']}: hit/order mismatch\n"
                f"EXPECTED {json.dumps(e, ensure_ascii=False)}\n"
                f"ACTUAL   {json.dumps(a, ensure_ascii=False)}"
            )
        for j, (x, y) in enumerate(zip(eh, ah)):
            xs, ys = x.get("score"), y.get("score")
            if (xs is None) != (ys is None):
                raise SystemExit(f"{e['id']} hit {j}: score presence differs")
            if xs is not None and not close(xs, ys):
                raise SystemExit(f"{e['id']} hit {j}: score mismatch {xs} vs {ys}")
    print("M4 result comparison passed")


if __name__ == "__main__":
    main()
