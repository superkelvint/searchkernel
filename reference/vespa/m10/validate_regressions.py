#!/usr/bin/env python3
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parent
PATH = ROOT / "config-regressions.json"
EXPECTED = {
    "attribute-only-custom-profile": {
        "fields": [("popularity", "integer", False, True)],
        "profile": ("popular", "attribute(popularity)"),
    },
    "text-custom-bm25-profile": {
        "fields": [("title", "text", True, False)],
        "profile": ("bm25", "bm25(title)"),
    },
}

root = json.load(open(PATH, encoding="utf-8"))
if root.get("version") != 1:
    raise SystemExit("M10 config regressions must use version 1")
cases = root.get("cases")
if not isinstance(cases, list) or [case.get("id") for case in cases] != list(EXPECTED):
    raise SystemExit("M10 config regression cases changed")

for case in cases:
    if case.get("schema", {}).get("document_type") != "typea":
        raise SystemExit(f"{case['id']}: document_type must be typea")
    expected = EXPECTED[case["id"]]
    fields = [
        (field.get("name"), field.get("type"), field.get("index", False), field.get("attribute", False))
        for field in case["schema"].get("fields", [])
    ]
    if fields != expected["fields"]:
        raise SystemExit(f"{case['id']}: regression field schema changed")
    profiles = case["schema"].get("rank_profiles")
    if not isinstance(profiles, list) or len(profiles) != 1:
        raise SystemExit(f"{case['id']}: exactly one custom rank profile is required")
    profile = profiles[0]
    if (profile.get("name"), profile.get("first_phase")) != expected["profile"]:
        raise SystemExit(f"{case['id']}: regression rank profile changed")
    if profile.get("match_features") != []:
        raise SystemExit(f"{case['id']}: regression match features must remain empty")

print("M10 config regressions valid")
