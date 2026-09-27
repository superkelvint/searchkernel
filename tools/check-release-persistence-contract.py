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
runner = (ROOT / ".github/workflows/ci-image-runner.yml").read_text()
if "python3 tools/check-release-persistence-contract.py" not in workflow:
    raise SystemExit("persistence workflow does not check the release persistence contract")
if "uses: ./.github/workflows/ci-image-runner.yml" not in workflow or "target: persistence" not in workflow:
    raise SystemExit("persistence workflow does not execute the exact-head native persistence lane")
if "persistence)" not in runner or "run_phase persistence-executable ./dev verify persistence" not in runner:
    raise SystemExit("CI image runner does not expose the executable persistence lane")
if "run_phase client-e2e-run" not in runner or "tools/check-client-contracts.py --run-e2e" not in runner:
    raise SystemExit("client-e2e lane does not execute public daemon/client evidence")
print(f"Release persistence contract PASS: storage generation {generation} has executable historical reopen coverage")
