#!/usr/bin/env python3
"""Fail closed when release persistence evidence drifts from the advertised contract."""
from pathlib import Path
import json, re, sys
ROOT = Path(__file__).resolve().parents[1]
manifest = (ROOT / "core/crates/searchkernel/src/index/manifest.rs").read_text()
docs = (ROOT / "docs/persistence-compatibility.md").read_text()
fixtures = ROOT / "reference/persistence/fixtures"
m = re.search(r"STORAGE_COMPATIBILITY_GENERATION: u32 = (\d+);", manifest)
if not m: raise SystemExit("cannot read current storage compatibility generation")
generation = int(m.group(1))
if f"current generation is `{generation}`" not in docs:
    raise SystemExit("docs/persistence-compatibility.md disagrees with current storage generation")
fixture_generations = set()
for path in fixtures.glob("manifest-v*-storage-v*/METADATA.json"):
    data = json.loads(path.read_text())
    fixture_generations.add(data["storage_compatibility_generation"])
if fixture_generations != {generation}:
    raise SystemExit(f"advertised generation {generation} lacks exact historical fixture coverage: {sorted(fixture_generations)}")
workflow = (ROOT / ".github/workflows/persistence-compatibility.yml").read_text()
required = [
    "cargo test -p searchkernel --test persistence_compatibility --features native -- --nocapture",
    "bash tools/check-persistence-sensitivity.sh",
    "python3 tools/check-release-persistence-contract.py",
]
for command in required:
    if command not in workflow:
        raise SystemExit(f"release persistence workflow does not execute required evidence: {command}")
if "Native verification policy" in workflow:
    raise SystemExit("release persistence workflow still treats executable native evidence as notice-only")
print(f"Release persistence contract PASS: storage generation {generation} has executable historical reopen coverage")
