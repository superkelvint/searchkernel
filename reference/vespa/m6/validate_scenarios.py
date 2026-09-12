#!/usr/bin/env python3
import json
import re
from pathlib import Path

ROOT = Path(__file__).resolve().parent
NAME = re.compile(r"^[A-Za-z_][A-Za-z0-9_]*$")
TYPES = {"text", "string", "integer", "float", "boolean"}

def fail(message):
    raise SystemExit(message)

scenarios = {}
for path in sorted((ROOT / "scenarios").glob("[0-9][0-9]-*.json")):
    obj = json.loads(path.read_text())
    if obj["id"] in scenarios:
        fail(f"duplicate scenario id: {obj['id']}")
    scenarios[obj["id"]] = obj
    if not NAME.fullmatch(obj["schema"]):
        fail(f"{path}: invalid schema/document type")
    seen = set()
    for field in obj["fields"]:
        name = field["name"]
        if not NAME.fullmatch(name):
            fail(f"{path}: invalid field name {name!r}")
        if name in seen:
            fail(f"{path}: duplicate field {name!r}")
        seen.add(name)
        if field["type"] not in TYPES:
            fail(f"{path}: unsupported type {field['type']!r}")
        idx = bool(field["index"])
        attr = bool(field["attribute"])
        if field["type"] == "text" and not idx:
            fail(f"{path}: text field must be indexed")
        if field["type"] != "text" and idx:
            fail(f"{path}: non-text field may not be indexed")
        if field["type"] != "text" and not attr:
            fail(f"{path}: non-text field must be an attribute")

base = scenarios.get("text-summary")
alt = scenarios.get("products-text-summary")
if base is None or alt is None:
    fail("document-type identity scenarios are missing")
if base["schema"] == alt["schema"]:
    fail("document-type identity scenarios must use different document types")
if base["fields"] != alt["fields"]:
    fail("document-type identity scenarios must have byte-equivalent field models")
if base["smoke"] != alt["smoke"]:
    fail("document-type identity scenarios must have equivalent smoke operations")

print(f"validated {len(scenarios)} M6 scenarios")
