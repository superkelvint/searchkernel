#!/usr/bin/env python3
import json,re
from pathlib import Path
ROOT=Path(__file__).resolve().parent
bit=re.compile(r"^0x[0-9a-f]{16}$")
scenarios=sorted(ROOT.glob("scenarios/[0-9][0-9]-*.json"))
for sp in scenarios:
    scenario=json.load(open(sp,encoding="utf-8")); sid=scenario["id"]
    ep=ROOT/"expected"/f"{sid}.json"
    if not ep.is_file(): raise SystemExit(f"M10 frozen expected missing: {ep}")
    out=json.load(open(ep,encoding="utf-8"))
    if set(out)!={"id","steps"} or out["id"]!=sid or not isinstance(out["steps"],list): raise SystemExit(f"{ep}: invalid root")
    expected_step_ids=[s["id"] for s in scenario["steps"]]
    if [s.get("id") for s in out["steps"]] != expected_step_ids: raise SystemExit(f"{ep}: output steps must preserve scenario step order")
    profiles={p["name"]:set(p["match_features"]) for p in scenario["schema"]["rank_profiles"]}
    for index,(actual,source) in enumerate(zip(out["steps"],scenario["steps"])):
        if set(actual)!={"id","op","total_hit_count","hits"} or actual.get("op")!="search": raise SystemExit(f"{ep}: step {index} shape changed")
        if not isinstance(actual["total_hit_count"],int) or actual["total_hit_count"]<len(actual["hits"]): raise SystemExit(f"{ep}: invalid total hit count")
        wanted=profiles[source["request"]["rank_profile"]]
        for hno,hit in enumerate(actual["hits"]):
            if set(hit)!={"document_id","score_bits","match_features"}: raise SystemExit(f"{ep}: hit {hno} shape changed")
            if not isinstance(hit["document_id"],str) or not bit.match(hit["score_bits"]): raise SystemExit(f"{ep}: hit {hno} score/id invalid")
            mf=hit["match_features"]
            if set(mf) != wanted: raise SystemExit(f"{ep}: hit {hno} match-feature names differ from selected profile: {set(mf)!r} != {wanted!r}")
            if any(not isinstance(v,str) or not bit.match(v) for v in mf.values()): raise SystemExit(f"{ep}: hit {hno} feature values must be exact IEEE-754 bits")

# The typed query-feature path and raw rank-property path are required to be
# semantically identical, not merely individually plausible.
ep=ROOT/"expected/raw-rank-property-equivalence.json"
if ep.is_file():
    out=json.load(open(ep,encoding="utf-8"))
    if len(out["steps"])!=2 or out["steps"][0]["hits"] != out["steps"][1]["hits"] or out["steps"][0]["total_hit_count"] != out["steps"][1]["total_hit_count"]:
        raise SystemExit("M10 typed query feature and raw rank property must yield bit-identical results")
print("M10 expected output valid")
