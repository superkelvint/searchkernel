#!/usr/bin/env python3
import json
from pathlib import Path

root = Path(__file__).resolve().parent
scenario_dir = root / "scenarios"
expected = {"crash-matrix.json", "repeated.json"}
actual = {path.name for path in scenario_dir.glob("*.json")}
if actual != expected:
    raise SystemExit(f"unexpected recovery scenarios: {sorted(actual)}")

with (scenario_dir / "crash-matrix.json").open(encoding="utf-8") as stream:
    matrix = json.load(stream)
if matrix != {
    "id": "cross-boundary-crash-matrix",
    "boundaries": ["product", "c5"],
    "operations": ["insert", "update", "delete", "close"],
    "phases": ["before", "after"],
    "matrix_repetitions": 3,
    "accepted_mutation_outcomes": ["pre", "post"],
    "identity_checks": True,
    "no_fresh_index_check": True,
    "real_process_abort": True,
}:
    raise SystemExit("crash-matrix scenario changed")

with (scenario_dir / "repeated.json").open(encoding="utf-8") as stream:
    repeated = json.load(stream)
if repeated != {
    "id": "cross-boundary-repeated-recovery",
    "boundaries": ["product", "c5"],
    "cycles": 20,
    "sequence": ["insert", "update", "delete", "close"],
    "reopen_after_every_crash": True,
    "identity_checks": True,
    "real_process_abort": True,
}:
    raise SystemExit("repeated scenario changed")
