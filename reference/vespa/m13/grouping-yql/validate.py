#!/usr/bin/env python3
import json
from pathlib import Path


ROOT = Path(__file__).resolve().parent
REPOSITORY = ROOT.parents[3]
manifest = json.loads((ROOT / "coverage.json").read_text(encoding="utf-8"))

assert manifest["version"] == 1
assert manifest["execution"] == "real-vespa"
cases = manifest["cases"]
assert {case["id"] for case in cases} == {
    "recursive-grouping",
    "root-labels-order-max-precision-aliases-filters",
    "yql-properties-order-and-grouping",
    "grouped-summary-hits",
    "grouping-continuations",
}

for case in cases:
    verifier = REPOSITORY / case["verifier"]
    assert verifier.exists(), f"missing verifier: {verifier}"
    assert case["coverage"], f"empty coverage description: {case['id']}"

by_id = {case["id"]: case for case in cases}
assert by_id["grouped-summary-hits"]["status"] == "executable"
assert by_id["grouping-continuations"]["status"] == "intentionally-deferred"
print("M13 grouping/YQL coverage manifest is valid")
