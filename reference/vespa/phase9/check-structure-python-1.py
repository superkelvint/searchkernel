#!/usr/bin/env python3
import pathlib
import re
import sys

source_root = pathlib.Path(sys.argv[1])
unity = (source_root / "abi.cc").read_text(encoding="utf-8")
for source_path in sorted(source_root.glob("*.cc")):
    if source_path.name != "abi.cc" and f'#include "{source_path.name}"' not in unity:
        raise SystemExit(f"native adapter source is not included by abi.cc: {source_path.name}")

source = unity

def body(name):
    match = re.search(r"\b" + re.escape(name) + r"\s*\([^)]*\)\s*(?:SK_NOEXCEPT\s*)?\{", source)
    if not match:
        raise SystemExit(f"missing native function: {name}")
    start = match.end() - 1
    depth = 0
    for index in range(start, len(source)):
        if source[index] == "{":
            depth += 1
        elif source[index] == "}":
            depth -= 1
            if depth == 0:
                return source[start:index + 1]
    raise SystemExit(f"unterminated native function: {name}")

for name, native_type in (
    ("sk_query_add_in_strings", "MultiTerm::Type::STRING"),
    ("sk_query_add_in_integers", "MultiTerm::Type::INTEGER"),
):
    function = body(name)
    if "add_in_term" not in function or native_type not in function:
        raise SystemExit(f"{name} does not construct the native Vespa InTerm")
    if "addOr" in function or "addNumberTerm" in function:
        raise SystemExit(f"{name} regressed to an equality disjunction")

if "addWeakAnd" not in body("sk_query_add_weak_and"):
    raise SystemExit("text lowering lost its native WeakAnd constructor")
