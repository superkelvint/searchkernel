#!/usr/bin/env python3
import json, math
from pathlib import Path

ROOT = Path(__file__).resolve().parent
FILES = sorted(ROOT.glob("scenarios/[0-9][0-9]-*.json"))
EXPECTED_IDS = [
    "bm25-title",
    "field-specific-bm25",
    "attribute-ranking-expression",
    "query-feature-controls-ranking",
    "raw-rank-property-equivalence",
    "arithmetic-ranking-expression",
    "native-rank-features",
    "multi-feature-inspection",
]
EXPECTED_PROFILES = {
    "bm25_title": ("bm25(title)", ["bm25(title)"]),
    "bm25_body": ("bm25(body)", ["bm25(body)"]),
    "popularity": ("attribute(popularity)", ["attribute(popularity)"]),
    "query_boost": ("query(boost) * attribute(popularity)", ["query(boost)", "attribute(popularity)"]),
    "expression": ("2 * attribute(popularity) + 3", ["attribute(popularity)"]),
    "native_title": ("nativeRank(title)", ["nativeRank(title)", "nativeFieldMatch(title)", "nativeProximity(title)"]),
    "inspect_all": ("bm25(title) + query(boost) * attribute(popularity)", ["bm25(title)", "query(boost)", "attribute(popularity)"]),
}

if len(FILES) != 8:
    raise SystemExit(f"M10 requires exactly 8 positive scenarios, found {len(FILES)}")
ids=[]
for path in FILES:
    obj=json.load(open(path,encoding="utf-8"))
    if obj.get("version") != 1: raise SystemExit(f"{path}: version must be 1")
    sid=obj.get("id"); ids.append(sid)
    schema=obj.get("schema",{})
    if schema.get("document_type") != "typea": raise SystemExit(f"{path}: document_type must be typea")
    fields={f.get("name"):(f.get("type"),f.get("index",False),f.get("attribute",False)) for f in schema.get("fields",[])}
    required={"title":("text",True,False),"body":("text",True,False),"popularity":("integer",False,True)}
    if fields != required: raise SystemExit(f"{path}: common M10 field schema changed: {fields!r}")
    profiles=schema.get("rank_profiles",[])
    got={p.get("name"):(p.get("first_phase"),p.get("match_features")) for p in profiles}
    if got != EXPECTED_PROFILES: raise SystemExit(f"{path}: frozen rank-profile corpus changed")
    docs=obj.get("documents")
    if not isinstance(docs,list) or [d.get("id") for d in docs] != [f"id:test:typea::d{i}" for i in range(1,5)]:
        raise SystemExit(f"{path}: frozen four-document ranking trap changed")
    for doc in docs:
        for field in ("title","body"):
            value=doc["fields"][field]
            if not isinstance(value.get("analysis"),list) or not value["analysis"]:
                raise SystemExit(f"{path}: {doc['id']} {field} must carry frozen M3 text annotations")
    steps=obj.get("steps")
    if not isinstance(steps,list) or not steps: raise SystemExit(f"{path}: steps required")
    seen=set()
    for step in steps:
        if step.get("op") != "search": raise SystemExit(f"{path}: M10 positives are ranking searches only")
        if not step.get("id") or step["id"] in seen: raise SystemExit(f"{path}: step ids must be unique")
        seen.add(step["id"])
        req=step.get("request",{})
        for key in ("query","rank_profile","query_features","rank_properties","limit","offset","sort","summary"):
            if key not in req: raise SystemExit(f"{path}: search request missing {key}")
        if req["summary"] != "none" or req["sort"] != []: raise SystemExit(f"{path}: M10 trap must rank by native score without docsums or hard sort")
        names=[x.get("name") for x in req["query_features"]]
        if len(names)!=len(set(names)): raise SystemExit(f"{path}: duplicate positive query feature")
        for x in req["query_features"]:
            if not isinstance(x.get("value"),(int,float)) or isinstance(x.get("value"),bool) or not math.isfinite(float(x["value"])):
                raise SystemExit(f"{path}: positive query features must be finite scalars")
        pnames=[x.get("name") for x in req["rank_properties"]]
        if len(pnames)!=len(set(pnames)): raise SystemExit(f"{path}: duplicate positive rank property")

if ids != EXPECTED_IDS: raise SystemExit(f"M10 positive scenario ids/order changed: {ids!r}")

s4=json.load(open(ROOT/"scenarios/04-query-feature-controls-ranking.json",encoding="utf-8"))
boosts=[step["request"]["query_features"][0]["value"] for step in s4["steps"]]
if boosts != [2.0,-1.0]: raise SystemExit("M10 query-feature trap must include positive and negative boost")
s5=json.load(open(ROOT/"scenarios/05-raw-rank-property-equivalence.json",encoding="utf-8"))
if len(s5["steps"]) != 2: raise SystemExit("M10 raw-property equivalence requires two searches")
if s5["steps"][0]["request"]["query_features"] != [{"name":"boost","value":2.0}]: raise SystemExit("M10 typed query-feature equivalence trap changed")
if s5["steps"][1]["request"]["rank_properties"] != [{"name":"boost","value":"2.0"}]: raise SystemExit("M10 raw rank-property equivalence trap changed")

errors=json.load(open(ROOT/"scenarios/errors.json",encoding="utf-8"))
cases=errors.get("cases",[])
if errors.get("version") != 1 or len(cases) != 14: raise SystemExit("M10 requires exactly 14 negative cases")
if len({c.get("id") for c in cases}) != 14: raise SystemExit("M10 negative ids must be unique")
required_errors={
 "invalid_rank_profile","duplicate_rank_profile","invalid_rank_expression",
 "invalid_match_feature","duplicate_match_feature","invalid_rank_property","duplicate_rank_property",
 "invalid_query_feature","duplicate_query_feature","rank_property_collision",
}
if {c.get("expected_error") for c in cases} != required_errors:
    raise SystemExit("M10 negative error-category set changed")
print("M10 scenarios valid")
