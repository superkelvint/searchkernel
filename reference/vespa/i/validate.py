#!/usr/bin/env python3
import json
import sys
from pathlib import Path

root = Path(__file__).resolve().parent
manifest = json.loads((root / "manifest.json").read_text())
scenario_paths = [root / path for path in manifest["scenarios"]]
if not scenario_paths:
    raise SystemExit("I-series corpus has no scenarios")

all_ids = set()
for scenario_path in scenario_paths:
    scenario = json.loads(scenario_path.read_text())
    if not scenario.get("id") or not scenario.get("fields", scenario.get("schema", {}).get("fields")):
        raise SystemExit(f"invalid scenario schema: {scenario_path}")
    case_ids = [case.get("id") for case in scenario.get("cases", [])]
    if len(case_ids) != len(set(case_ids)):
        raise SystemExit(f"duplicate case id in {scenario_path}")
    overlap = all_ids.intersection(case_ids)
    if overlap:
        raise SystemExit(f"duplicate case ids across scenarios: {sorted(overlap)}")
    all_ids.update(case_ids)
    expected_path = root / "expected" / f"{scenario['id']}.json"
    if not expected_path.is_file():
        raise SystemExit(f"missing expected output: {expected_path}")
    expected = json.loads(expected_path.read_text())
    expected_ids = [case.get("id") for case in expected.get("cases", [])]
    if expected_ids != case_ids:
        raise SystemExit(f"expected case order differs from scenario: {scenario_path}")

    expected_text = expected_path.read_text()
    for detail_code in manifest["unsupported_detail_codes"]:
        if detail_code not in expected_text:
            raise SystemExit(
                f"unsupported detail code is not exercised by frozen output: {detail_code}"
            )

required_ops = {
    "describe", "insert", "upsert", "patch", "get", "get_projection", "delete",
    "search", "analyze_field", "analyze_analyzer", "inspect", "close", "reopen",
    "closed_search", "create_duplicate", "open_missing", "create_schema",
}
seen_ops = {case["op"] for path in scenario_paths for case in json.loads(path.read_text())["cases"]}
missing = required_ops - seen_ops
if missing:
    raise SystemExit(f"I-series operation coverage is incomplete: {sorted(missing)}")

print(f"I-series corpus valid: {len(all_ids)} cases")
