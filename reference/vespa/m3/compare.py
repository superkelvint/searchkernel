#!/usr/bin/env python3
import json
import sys
from pathlib import Path


def load(path: Path):
    out = []
    with path.open("r", encoding="utf-8") as f:
        for n, line in enumerate(f, 1):
            line = line.strip()
            if line:
                try:
                    out.append(json.loads(line))
                except json.JSONDecodeError as e:
                    raise SystemExit(f"{path}:{n}: invalid JSON: {e}")
    return out


def main() -> int:
    if len(sys.argv) != 3:
        print(f"usage: {sys.argv[0]} EXPECTED.jsonl ACTUAL.jsonl", file=sys.stderr)
        return 2
    expected_path, actual_path = map(Path, sys.argv[1:])
    expected = load(expected_path)
    actual = load(actual_path)
    if len(expected) != len(actual):
        print(f"count mismatch: {len(expected)} expected, {len(actual)} actual", file=sys.stderr)
        return 1
    for i, (e, a) in enumerate(zip(expected, actual), 1):
        if e != a:
            print(f"mismatch at case #{i}: expected id={e.get('id')!r}, actual id={a.get('id')!r}", file=sys.stderr)
            print("EXPECTED:", json.dumps(e, ensure_ascii=False, indent=2), file=sys.stderr)
            print("ACTUAL:", json.dumps(a, ensure_ascii=False, indent=2), file=sys.stderr)
            return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
