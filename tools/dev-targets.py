#!/usr/bin/env python3
import json
import pathlib
import sys

ROOT = pathlib.Path(__file__).resolve().parents[1]
REGISTRY = ROOT / "build" / "targets.json"

def _reject_duplicate_keys(pairs):
    result = {}
    for key, value in pairs:
        if key in result:
            raise ValueError(f"duplicate key: {key}")
        result[key] = value
    return result

def load():
    return json.loads(REGISTRY.read_text(), object_pairs_hook=_reject_duplicate_keys)

def entries(kind):
    return load()[kind]

def canonical(kind, name):
    for target, meta in entries(kind).items():
        if name == target or name in meta.get("aliases", []):
            return target
    return None

def _matches_area(normalized, prefix):
    """Match exact files exactly and directory prefixes only at path boundaries."""
    target = prefix.rstrip("/")
    if normalized == target:
        return True
    return prefix.endswith("/") and normalized.startswith(prefix)

def explain(path):
    """Print the most specific registered ownership guidance for a path."""
    parsed = pathlib.PurePosixPath(path.replace("\\", "/"))
    if parsed.is_absolute() or ".." in parsed.parts:
        print(f"path must be repository-relative: {path}", file=sys.stderr)
        raise SystemExit(2)
    normalized = parsed.as_posix()
    matches = [
        (prefix, metadata)
        for prefix, metadata in entries("change_areas").items()
        if _matches_area(normalized, prefix)
    ]
    if not matches:
        print(f"no change area registered for path: {path}", file=sys.stderr)
        print("add an ownership entry to build/targets.json", file=sys.stderr)
        raise SystemExit(2)
    prefix, metadata = max(matches, key=lambda item: len(item[0]))
    print(f"Area: {metadata['area']}")
    print(f"Owner: {metadata['owner']}")
    print(f"Architecture: {metadata['architecture']}")
    print("Read: " + ", ".join(metadata["read"]))
    print(f"Focused check: {command_for(metadata['focused_check'])}")
    print(f"Strong check: {command_for(metadata['strong_check'])}")
    print(f"Native base rebuild: {'yes' if metadata['native_base_rebuild'] else 'no'}")
    print(f"Generated file: {'yes' if metadata['generated'] else 'no'}")
    for rule in metadata.get("rules", []):
        print(f"Rule: {rule}")

def command_for(reference):
    kind, target = reference.split(":", 1)
    if kind == "command":
        return f"./dev {target}"
    if kind in {"test", "verify"}:
        return f"./dev {kind} {target}"
    raise ValueError(f"unsupported check reference: {reference}")

def main():
    if len(sys.argv) >= 2 and sys.argv[1] == "explain":
        if len(sys.argv) != 3:
            raise SystemExit("usage: dev-targets.py explain <path>")
        explain(sys.argv[2])
        return
    if len(sys.argv) < 3 or sys.argv[1] not in {"list", "resolve"}:
        raise SystemExit("usage: dev-targets.py list <test|verify|universe> | resolve <test|verify> <target> | explain <path>")
    action, kind = sys.argv[1:3]
    if kind not in {"test", "verify", "universe"}:
        raise SystemExit(f"unknown target kind: {kind}")
    if action == "resolve" and kind == "universe":
        raise SystemExit("universe is a list-only verifier collection")
    if action == "list":
        source = entries("verify") if kind == "universe" else entries(kind)
        for target, meta in source.items():
            if kind == "universe" and meta.get("universe") != "semantic":
                continue
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
