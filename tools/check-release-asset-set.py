#!/usr/bin/env python3
"""Verify the complete candidate asset set before release publication."""

import hashlib
import json
import sys
from pathlib import Path

root = Path(sys.argv[1])
manifest = json.loads((root / "manifest.json").read_text(encoding="utf-8"))
entries = manifest.get("artifacts")
if not isinstance(entries, list) or not entries:
    raise SystemExit("release manifest has no artifacts")

names = []
for entry in entries:
    if not isinstance(entry, dict):
        raise SystemExit("invalid release artifact entry")
    name = entry.get("file")
    if not isinstance(name, str) or not name or Path(name).name != name:
        raise SystemExit(f"invalid release artifact filename: {name!r}")
    if name in names or name == "manifest.json":
        raise SystemExit(f"duplicate release artifact filename: {name}")
    names.append(name)
    path = root / name
    if not path.is_file():
        raise SystemExit(f"release artifact is missing: {name}")
    digest = hashlib.sha256(path.read_bytes()).hexdigest()
    if digest != entry.get("sha256") or path.stat().st_size != entry.get("bytes"):
        raise SystemExit(f"release artifact checksum or size mismatch: {name}")

if not any(name.startswith("searchkernel-rust-") and name.endswith(".tar.gz") for name in names):
    raise SystemExit("release manifest lacks the Rust source bundle")
if manifest.get("runtime", {}).get("file") not in names:
    raise SystemExit("release runtime is absent from artifact set")
if {p.name for p in root.iterdir() if p.is_file()} != set(names) | {"manifest.json"}:
    raise SystemExit("release directory differs from manifest artifact set")

if "--names" in sys.argv[2:]:
    print("\n".join(names))
