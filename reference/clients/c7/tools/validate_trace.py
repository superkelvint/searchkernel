#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
from pathlib import Path


def load_events(path: Path):
    events = []
    for line in path.read_text().splitlines():
        line = line.strip()
        if line:
            events.append(json.loads(line))
    return events


def expect_eq(summary: dict, key: str, expected: int):
    actual = int(summary.get(key, 0))
    if actual != expected:
        raise SystemExit(f"expected {key}={expected}, got {actual}")


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("trace", type=Path)
    ap.add_argument("--expect-opens", type=int)
    ap.add_argument("--expect-closes", type=int)
    ap.add_argument("--expect-calls", type=int)
    ap.add_argument("--expect-allocs", type=int)
    ap.add_argument("--expect-frees", type=int)
    ap.add_argument("--require-abi-before-native-use", action="store_true")
    args = ap.parse_args()

    events = load_events(args.trace)
    summaries = [e for e in events if e.get("event") == "summary"]
    if not summaries:
        raise SystemExit("C7 trace has no summary record")
    s = summaries[-1]

    for key, expected in (
        ("open", args.expect_opens),
        ("close", args.expect_closes),
        ("call", args.expect_calls),
        ("alloc", args.expect_allocs),
        ("free", args.expect_frees),
    ):
        if expected is not None:
            expect_eq(s, key, expected)

    if s.get("alloc") != s.get("free"):
        raise SystemExit(f"native response leak: alloc={s.get('alloc')} free={s.get('free')}")
    if int(s.get("outstanding", 0)) != 0:
        raise SystemExit(f"outstanding native buffers: {s.get('outstanding')}")
    if int(s.get("double_free", 0)) != 0:
        raise SystemExit(f"double frees observed: {s.get('double_free')}")

    native_double_close = sum(1 for e in events if e.get("event") == "native_double_close")
    if native_double_close:
        raise SystemExit(f"native double-close events observed: {native_double_close}")

    if args.require_abi_before_native_use:
        abi_positions = [i for i, e in enumerate(events) if e.get("event") == "abi_version"]
        native_positions = [
            i for i, e in enumerate(events) if e.get("event") in {"open", "call"}
        ]
        if native_positions:
            if not abi_positions or min(abi_positions) > min(native_positions):
                raise SystemExit("C5 ABI version was not checked before first native open/call")

    print(
        json.dumps(
            {
                "status": "PASS",
                "summary": s,
                "native_double_close": native_double_close,
            },
            sort_keys=True,
        )
    )


if __name__ == "__main__":
    main()
