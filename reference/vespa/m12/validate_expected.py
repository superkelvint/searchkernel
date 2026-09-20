#!/usr/bin/env python3
import json
from pathlib import Path
ROOT=Path(__file__).resolve().parent
positive=sorted((ROOT/"scenarios").glob("[0-9][0-9]-*.json"))
expected_dir=ROOT/"expected"
for scenario in positive:
    sid=json.load(open(scenario,encoding="utf-8"))["id"]
    path=expected_dir/f"{sid}.json"
    if not path.is_file(): raise SystemExit(f"missing M12 expected file: {path.name}")
    obj=json.load(open(path,encoding="utf-8"))
    if obj.get("id")!=sid: raise SystemExit(f"{path.name}: id mismatch")
    steps=obj.get("steps")
    if not isinstance(steps,list) or not steps: raise SystemExit(f"{path.name}: missing steps")
    for step in steps:
        hits=step.get("hits")
        if not isinstance(hits,list) or not hits: raise SystemExit(f"{path.name}: every positive step needs ordered hits")
        if len(hits)!=len(set(hits)): raise SystemExit(f"{path.name}: duplicate hit id")
# Guard the hand-reviewable semantic trap.
orders={p.stem:[s["hits"] for s in json.load(open(p,encoding="utf-8"))["steps"]] for p in expected_dir.glob("*.json")}
if orders["query-vector-changes-order"][0] == orders["query-vector-changes-order"][1]:
    raise SystemExit("M12 goldens must prove changing tensor cells changes order")
if orders["multiple-query-tensors"][0][0] != "id:ns:typea::y":
    raise SystemExit("M12 named-input golden no longer proves second tensor selection")
print("M12 expected outputs valid")
