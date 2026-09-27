#!/usr/bin/env python3
from pathlib import Path
source = (Path(__file__).resolve().parents[1] / "core/native/vespa/engine.cc").read_text()
start = source.index("    void put_document(const ValueDocument &document) {")
end = source.index("    void update_document(const ValueUpdate &update) {", start)
put = source[start:end]
if "has_tensor" in put:
    raise SystemExit("tensor put still has a tensor-specific visibility bypass")
required = [
    "wait_for_source_state(id, true, expected_fields(document), true);",
    "if (has_struct_array || has_position)",
    "value_matches()",
    'failpoint("mutation_before_visibility_wait")',
]
for token in required:
    if token not in put:
        raise SystemExit(f"tensor put visibility contract missing: {token}")
print("Tensor put visibility contract PASS")
