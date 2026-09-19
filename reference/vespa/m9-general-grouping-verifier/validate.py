#!/usr/bin/env python3
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parent
scenario = json.loads((ROOT / "scenarios/01-recursive-two-level.json").read_text())
expected = json.loads((ROOT / "expected/recursive-two-level.json").read_text())

assert scenario["version"] == 1
assert scenario["id"] == "recursive-two-level"
assert scenario["schema"]["document_type"] == "typea"
assert [f["name"] for f in scenario["schema"]["fields"]] == [
    "category", "qty", "price", "active"
]
assert len(scenario["levels"]) == 2
assert scenario["levels"][0] == {
    "group_by": "category",
    "outputs": ["count", {"average": "price"}],
}
assert scenario["levels"][1] == {
    "group_by": "qty",
    "outputs": ["count", {"average": "price"}],
}
assert len(scenario["documents"]) == 4
assert [
    (d["fields"]["category"]["string"],
     d["fields"]["qty"]["integer"],
     d["fields"]["price"]["float"])
    for d in scenario["documents"]
] == [("A", 1, 10.0), ("A", 1, 20.0), ("A", 2, 30.0), ("B", 1, 40.0)]
assert expected["id"] == scenario["id"]
assert len(expected["groups"]) == 2
print("recursive grouping verifier inputs valid")
