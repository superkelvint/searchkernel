#!/usr/bin/env python3
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


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("trace", type=Path)
    ap.add_argument("--min-calls", type=int, default=0)
    ap.add_argument("--expect-opens", type=int)
    ap.add_argument("--expect-closes", type=int)
    args = ap.parse_args()

    events = load_events(args.trace)
    summaries = [e for e in events if e.get("event") == "summary"]
    if not summaries:
        raise SystemExit("C7 trace has no summary record")
    s = summaries[-1]

    if s.get("call", 0) < args.min_calls:
        raise SystemExit(f"expected at least {args.min_calls} calls, got {s.get('call')}")
    if args.expect_opens is not None and s.get("open") != args.expect_opens:
        raise SystemExit(f"expected {args.expect_opens} opens, got {s.get('open')}")
    if args.expect_closes is not None and s.get("close") != args.expect_closes:
        raise SystemExit(f"expected {args.expect_closes} closes, got {s.get('close')}")
    if s.get("alloc") != s.get("free"):
        raise SystemExit(f"native response leak: alloc={s.get('alloc')} free={s.get('free')}")
    if s.get("outstanding") != 0:
        raise SystemExit(f"outstanding native buffers: {s.get('outstanding')}")
    if s.get("double_free") != 0:
        raise SystemExit(f"double frees observed: {s.get('double_free')}")

    print(json.dumps({"status": "PASS", "summary": s}, sort_keys=True))


if __name__ == "__main__":
    main()
