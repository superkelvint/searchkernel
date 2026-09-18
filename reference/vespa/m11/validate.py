#!/usr/bin/env python3
import json
import sys
from pathlib import Path

root = Path(__file__).resolve().parent
scenarios = root / "scenarios"
required = {
    "lifecycle.json",
    "malformed.json",
    "failures.json",
    "reopen.json",
    "concurrency.json",
    "advanced.json",
    "adapter.json",
}
actual = {path.name for path in scenarios.glob("*.json")}
if actual != required:
    raise SystemExit(f"unexpected M11 scenario set: {sorted(actual)}")

for path in sorted(scenarios.glob("*.json")):
    with path.open(encoding="utf-8") as stream:
        value = json.load(stream)
    if not isinstance(value, dict) or not value.get("id", "").startswith("m11-"):
        raise SystemExit(f"invalid scenario identity: {path}")

checks = {
    "lifecycle.json": {"fresh_open_close": 1000, "reopen": 250},
    "reopen.json": {"cycles": 250},
    "concurrency.json": {
        "mutation_workers": 8,
        "search_workers": 4,
        "aggregate_operations": 800000,
    },
}
for filename, expected in checks.items():
    with (scenarios / filename).open(encoding="utf-8") as stream:
        value = json.load(stream)
    for key, required_value in expected.items():
        if value.get(key) != required_value:
            raise SystemExit(f"{filename}: {key} is not frozen at {required_value}")

seed = (scenarios / "stress-seeds.txt").read_text(encoding="utf-8").strip()
if seed != "seed=0x4d1120260914":
    raise SystemExit("stress seed corpus changed")

for filename, keys in {
    "advanced.json": ["direct_engine", "sorting", "summaries", "results", "grouping", "facets", "attribute_mutation", "ranking", "match_features"],
    "adapter.json": ["real_vespa_backend", "facets", "sort", "projection"],
}.items():
    with (scenarios / filename).open(encoding="utf-8") as stream:
        value = json.load(stream)
    if any(value.get(key) is not True for key in keys):
        raise SystemExit(f"{filename}: expanded M11 coverage is incomplete")

with (scenarios / "failures.json").open(encoding="utf-8") as stream:
    failures = json.load(stream)
if "malformed_result" not in failures.get("actions", []) or not {
    "document_result_accessors",
    "summary_result_accessors",
    "match_feature_accessors",
    "group_result_accessors",
    "aggregation_result_accessors",
}.issubset(failures.get("boundaries", [])):
    raise SystemExit("failures.json: result accessor fault coverage is incomplete")

with (root / "expected/invariants.json").open(encoding="utf-8") as stream:
    invariants = json.load(stream)
if invariants != {
    "fd_slack": 2,
    "pss_absolute_slack_bytes": 67108864,
    "pss_relative_slack": 0.15,
    "query_depth": 256,
    "query_nodes": 65536,
    "yql_bytes": 4194304,
    "disk_file_slack": 2,
    "disk_byte_slack": 65536,
}:
    raise SystemExit("M11 safety invariants changed")
