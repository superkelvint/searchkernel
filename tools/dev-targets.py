#!/usr/bin/env python3
import json
import pathlib
import sys

ROOT = pathlib.Path(__file__).resolve().parents[1]
REGISTRY = ROOT / "build" / "targets.json"

def load():
    return json.loads(REGISTRY.read_text())

def entries(kind):
    return load()[kind]

def canonical(kind, name):
    for target, meta in entries(kind).items():
        if name == target or name in meta.get("aliases", []):
            return target
    return None

def main():
    if len(sys.argv) < 3 or sys.argv[1] not in {"list", "resolve"}:
        raise SystemExit("usage: dev-targets.py list <test|verify> | resolve <test|verify> <target>")
    action, kind = sys.argv[1:3]
    if kind not in {"test", "verify"}:
        raise SystemExit(f"unknown target kind: {kind}")
    if action == "list":
        for target, meta in entries(kind).items():
            prefix = "  " if kind == "verify" else ""
            print(f"{prefix}{target}")
        return
    if len(sys.argv) != 4:
        raise SystemExit("resolve requires a target")
    target = canonical(kind, sys.argv[3])
    if target is None:
        print(f"unknown {kind} target: {sys.argv[3]}", file=sys.stderr)
        print(f"run './dev {kind} --list' for supported targets", file=sys.stderr)
        raise SystemExit(2)
    print(target)

if __name__ == "__main__":
    main()
