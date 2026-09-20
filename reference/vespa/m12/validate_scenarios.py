#!/usr/bin/env python3
import json, math
from pathlib import Path
ROOT=Path(__file__).resolve().parent
files=sorted((ROOT/"scenarios").glob("[0-9][0-9]-*.json"))
if len(files)!=4: raise SystemExit("M12 requires exactly four positive scenarios")
ids=[]
for path in files:
    root=json.load(open(path,encoding="utf-8"))
    if root.get("version")!=1: raise SystemExit(f"{path.name}: version must be 1")
    sid=root.get("id")
    if not sid: raise SystemExit(f"{path.name}: missing id")
    ids.append(sid)
    fixture=root.get("fixture",{})
    field=fixture.get("tensor_field",{})
    if field != {"name":"embedding","type_spec":"tensor<float>(x[3])","distance_metric":"euclidean","hnsw":False}:
        raise SystemExit(f"{path.name}: frozen tensor fixture changed")
    docs=fixture.get("documents",[])
    expected_docs={
        "id:ns:typea::x":[1.0,0.0,0.0],
        "id:ns:typea::mid":[0.8,0.2,0.0],
        "id:ns:typea::y":[0.0,1.0,0.0],
    }
    if {d.get("id"):d.get("values") for d in docs} != expected_docs:
        raise SystemExit(f"{path.name}: frozen vectors changed")
    for step in root.get("steps",[]):
        if step.get("document_field")!="embedding": raise SystemExit(f"{path.name}: document field changed")
        if not step.get("query_tensor"): raise SystemExit(f"{path.name}: missing query tensor name")
        if step.get("limit")!=3: raise SystemExit(f"{path.name}: limit must be 3")
        names=[]
        for inp in step.get("inputs",[]):
            names.append(inp.get("name"))
            if inp.get("kind")!="dense": raise SystemExit(f"{path.name}: positive inputs must be dense")
            if inp.get("type_spec")!="tensor<float>(x[3])": raise SystemExit(f"{path.name}: positive type spec changed")
            vals=inp.get("values",[])
            if len(vals)!=3 or not all(isinstance(v,(int,float)) and math.isfinite(v) for v in vals):
                raise SystemExit(f"{path.name}: invalid positive dense cells")
        if step["query_tensor"] not in names: raise SystemExit(f"{path.name}: query tensor reference missing")
        if len(names)!=len(set(names)): raise SystemExit(f"{path.name}: duplicate positive input names")
        for qf in step.get("scalar_query_features",[]):
            if not qf.get("name") or not math.isfinite(qf.get("value",float("nan"))):
                raise SystemExit(f"{path.name}: invalid scalar feature")
if len(set(ids))!=len(ids): raise SystemExit("M12 positive scenario ids must be unique")
if ids != ["exact-near-far","query-vector-changes-order","multiple-query-tensors","scalar-and-tensor-inputs"]:
    raise SystemExit("M12 positive scenario set changed")

s2=json.load(open(ROOT/"scenarios/02-query-vector-changes-order.json",encoding="utf-8"))
if s2["steps"][0]["inputs"][0]["values"] == s2["steps"][1]["inputs"][0]["values"]:
    raise SystemExit("M12 query-vector trap must change tensor cells")
s3=json.load(open(ROOT/"scenarios/03-multiple-query-tensors.json",encoding="utf-8"))
step=s3["steps"][0]
if [x["name"] for x in step["inputs"]] != ["q_x","q_y"] or step["query_tensor"]!="q_y":
    raise SystemExit("M12 named-input trap changed")
s4=json.load(open(ROOT/"scenarios/04-scalar-and-tensor-inputs.json",encoding="utf-8"))
if s4["steps"][0]["scalar_query_features"] != [{"name":"boost","value":2.0}]:
    raise SystemExit("M12 scalar/tensor coexistence trap changed")

errors=json.load(open(ROOT/"scenarios/errors.json",encoding="utf-8"))
cases=errors.get("cases",[])
if errors.get("version")!=1 or len(cases)!=19: raise SystemExit("M12 requires exactly 19 negative cases")
if len({c.get("id") for c in cases})!=19: raise SystemExit("M12 negative ids must be unique")
required={
 "invalid_query_tensor","duplicate_query_tensor","query_input_collision","query_tensor_missing",
 "query_tensor_type_mismatch","query_tensor_field_invalid","expression_query_input_not_supported",
 "sparse_query_tensor_not_supported","mixed_query_tensor_not_supported","non_finite_tensor_cell"
}
if {c.get("expected_error") for c in cases} != required:
    raise SystemExit("M12 negative error-category set changed")
if {c.get("path") for c in cases}!={"low_level","portable"}:
    raise SystemExit("M12 errors must exercise low-level and portable paths")
print("M12 scenarios valid")
