#!/usr/bin/env python3
import argparse
import json
import subprocess
import sys
from pathlib import Path


def protocol(cases_path: Path) -> bytes:
    lines = []
    seen = set()
    with cases_path.open("r", encoding="utf-8") as f:
        for lineno, raw in enumerate(f, 1):
            raw = raw.strip()
            if not raw:
                continue
            case = json.loads(raw)
            case_id = case["id"]
            if case_id in seen:
                raise SystemExit(f"duplicate scenario id {case_id!r} at line {lineno}")
            seen.add(case_id)
            if not case_id or any(ch not in "abcdefghijklmnopqrstuvwxyz0123456789-" for ch in case_id):
                raise SystemExit(f"invalid scenario id {case_id!r}: use [a-z0-9-]+")
            stemming = case["stemming"]
            if stemming not in ("none", "default"):
                raise SystemExit(f"unsupported stemming {stemming!r} in {case_id}")
            remove = "1" if bool(case["remove_accents"]) else "0"
            text_hex = case["input"].encode("utf-8").hex()
            lines.append(f"{case_id}\t{stemming}\t{remove}\t{text_hex}\n")
    return "".join(lines).encode("ascii")


def main() -> int:
    p = argparse.ArgumentParser()
    p.add_argument("--cases", required=True, type=Path)
    p.add_argument("--output", required=True, type=Path)
    p.add_argument("command", nargs=argparse.REMAINDER)
    args = p.parse_args()
    if not args.command:
        p.error("missing command after --output")

    proc = subprocess.run(
        args.command,
        input=protocol(args.cases),
        stdout=subprocess.PIPE,
        stderr=sys.stderr,
        check=False,
    )
    if proc.returncode != 0:
        return proc.returncode

    # Validate that the command emitted one valid canonical JSON object per case.
    input_ids = []
    with args.cases.open("r", encoding="utf-8") as f:
        for raw in f:
            raw = raw.strip()
            if raw:
                input_ids.append(json.loads(raw)["id"])

    output_lines = [line for line in proc.stdout.decode("utf-8").splitlines() if line.strip()]
    if len(output_lines) != len(input_ids):
        raise SystemExit(f"expected {len(input_ids)} output lines, got {len(output_lines)}")

    canonical = []
    for expected_id, line in zip(input_ids, output_lines):
        obj = json.loads(line)
        if obj.get("id") != expected_id:
            raise SystemExit(f"output id {obj.get('id')!r} does not match {expected_id!r}")
        if not isinstance(obj.get("terms"), list):
            raise SystemExit(f"{expected_id}: missing terms list")
        canonical.append(json.dumps(obj, ensure_ascii=False, separators=(",", ":")))

    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text("\n".join(canonical) + "\n", encoding="utf-8")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
