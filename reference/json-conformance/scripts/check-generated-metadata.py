#!/usr/bin/env python3
import json
from pathlib import Path

here = Path(__file__).resolve().parent.parent
root = here.parent.parent
generated = here / "generated"

manifest = json.loads((here / "expected/fixture-manifest.json").read_text())
if manifest.get("format_version") != 1:
    raise SystemExit("invalid fixture manifest format")


def fixture_names(directory, suffix):
    return sorted(path.stem for path in directory.glob(f"*.{suffix}"))


for kind, suffix in [("requests", "skrq"), ("responses", "skrs")]:
    expected = manifest.get(kind)
    if not isinstance(expected, list) or expected != sorted(set(expected)):
        raise SystemExit(f"{kind} fixture manifest must be a sorted unique list")
    actual = fixture_names(
        root / "core/crates/searchkernel-json/tests/fixtures" / kind, suffix
    )
    if actual != expected:
        raise SystemExit(
            f"{kind} fixture set mismatch: expected {len(expected)}, found {len(actual)}"
        )


for retired in ["manifest.json", "operation-map.json", "API.md", "requests", "responses"]:
    if (generated / retired).exists():
        raise SystemExit(f"retired generated artifact remains: generated/{retired}")

schema_model = json.loads((generated / "schema-model.json").read_text())
if not isinstance(schema_model.get("enums"), list) or not isinstance(
    schema_model.get("roots"), list
):
    raise SystemExit("schema-model.json is missing BFBS inventory sections")

print("C6 generated metadata/fixture coverage PASS")
