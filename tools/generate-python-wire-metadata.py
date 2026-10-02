#!/usr/bin/env python3
"""Generate private Python wire enum-field metadata from canonical BFBS."""

from __future__ import annotations

import sys
from pathlib import Path

from searchkernel_bfbs import ReflectedSchema


def render(bfbs: Path) -> str:
    schema = ReflectedSchema(bfbs)
    lines = [
        '"""Generated V1 codec metadata; do not edit."""',
        "",
        "ENUM_FIELDS = {",
    ]
    for obj in schema.wire_objects():
        specs = [
            (schema.field_name(field), spec)
            for field in schema.fields(obj)
            if (spec := schema.enum_spec(field)) is not None
        ]
        if not specs:
            continue
        entries = ", ".join(f"{field!r}: {enum_name!r}" for field, enum_name in specs)
        lines.append(f"    {schema.short_name(obj)!r}: {{{entries}}},")
    lines.extend(["}", ""])
    return "\n".join(lines)


def main() -> int:
    if len(sys.argv) != 3:
        raise SystemExit(
            "usage: generate-python-wire-metadata.py SEARCHKERNEL_RESPONSE_BFBS OUTPUT"
        )
    output = Path(sys.argv[2])
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text(render(Path(sys.argv[1])), encoding="utf-8")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
