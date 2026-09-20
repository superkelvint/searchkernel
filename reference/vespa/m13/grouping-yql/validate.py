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
    "grouped-summary-and-continuation",
}

for case in cases:
    verifier = REPOSITORY / case["verifier"]
    assert verifier.exists(), f"missing verifier: {verifier}"
    assert case["coverage"], f"empty coverage description: {case['id']}"

blocked = [case for case in cases if case.get("status") == "blocked-until-native-support"]
assert [case["id"] for case in blocked] == ["grouped-summary-and-continuation"]
print("M13 grouping/YQL coverage manifest is valid")
