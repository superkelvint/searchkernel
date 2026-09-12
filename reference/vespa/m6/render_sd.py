#!/usr/bin/env python3
import json
import re
import sys
from pathlib import Path

NAME = re.compile(r"^[A-Za-z_][A-Za-z0-9_]*$")
TYPE_MAP = {
    "text": "string",
    "string": "string",
    "integer": "int",
    "float": "double",
    "boolean": "bool",
}

def fail(msg):
    raise SystemExit(msg)

def validate(obj):
    if not NAME.match(obj["schema"]):
        fail(f"invalid schema name: {obj['schema']!r}")
    seen = set()
    for field in obj["fields"]:
        name = field["name"]
        if not NAME.match(name):
            fail(f"invalid field name: {name!r}")
        if name in seen:
            fail(f"duplicate field: {name}")
        seen.add(name)
        kind = field["type"]
        if kind not in TYPE_MAP:
            fail(f"unsupported field type: {kind}")
        idx = bool(field.get("index"))
        attr = bool(field.get("attribute"))
        if kind == "text" and not idx:
            fail(f"text field {name} must be indexed")
        if kind != "text" and idx:
            fail(f"non-text field {name} may not be indexed in M6")
        if kind != "text" and not attr:
            fail(f"non-text field {name} must be an attribute in M6")

def render(obj):
    validate(obj)
    out = [f"schema {obj['schema']} {{", "", f"  document {obj['schema']} {{"]
    for field in obj["fields"]:
        ops = []
        if field.get("index"):
            ops.append("index")
        if field.get("attribute"):
            ops.append("attribute")
        if field.get("summary"):
            ops.append("summary")
        out.extend([
            f"    field {field['name']} type {TYPE_MAP[field['type']]} {{",
            f"      indexing: {' | '.join(ops)}",
            "    }",
        ])
    out.extend(["  }", "}", ""])
    return "\n".join(out)

def main():
    if len(sys.argv) != 3:
        fail("usage: render_sd.py SCENARIO.json OUTPUT.sd")
    obj = json.loads(Path(sys.argv[1]).read_text())
    Path(sys.argv[2]).write_text(render(obj))

if __name__ == "__main__":
    main()
