#!/usr/bin/env python3
import json
import sys

if len(sys.argv) != 4:
    raise SystemExit("usage: make-error-definition.py BASE.json ERRORS.json CASE_ID")

with open(sys.argv[1], encoding="utf-8") as handle:
    base = json.load(handle)
with open(sys.argv[2], encoding="utf-8") as handle:
    errors = json.load(handle)
case = next((item for item in errors["cases"] if item["id"] == sys.argv[3]), None)
if case is None:
    raise SystemExit("unknown case")

mutation = case["mutation"]
field = base["fields"][0]

# The two type-negative cases must isolate the feature under test.  The positive
# fixture has both HNSW and a distance metric, so remove the unrelated feature
# before changing the field to string.
if case["id"] == "portable-hnsw-non-tensor":
    field.get("attribute", {}).pop("distance_metric", None)
elif case["id"] == "portable-metric-non-tensor":
    if "index" in field:
        field["index"].pop("hnsw", None)
        field["index"]["enabled"] = {"value": False}
    field["searchable"] = {"value": False}

if mutation.get("field_type") == "string":
    field["data_type"] = {
        "kind_type": "PrimitiveDataType",
        "kind": {"kind": "String"},
    }
    field["searchable"] = {"value": False}

if mutation.get("hnsw") is True:
    field.setdefault("index", {})["enabled"] = {"value": True}
    field["index"].setdefault("hnsw", {})

if mutation.get("distance_metric"):
    field.setdefault("attribute", {"enabled": {"value": True}})["distance_metric"] = "Euclidean"

if "hnsw_option" in mutation:
    option = mutation["hnsw_option"]
    field["index"]["hnsw"]["options"] = [
        {
            "key": option["key"],
            "value": {"kind_type": "StringValue", "kind": {"value": option["value"]}},
        }
    ]

if "attribute_option" in mutation:
    option = mutation["attribute_option"]
    field["attribute"]["options"] = [
        {
            "key": option["key"],
            "value": {"kind_type": "StringValue", "kind": {"value": option["value"]}},
        }
    ]

print(json.dumps(base, separators=(",", ":")))
