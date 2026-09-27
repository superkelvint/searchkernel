#!/usr/bin/env python3
"""Spike-only linker wrapper for Candidate A.

All ordinary Cargo links are passed through unchanged. Final executables that
contain searchkernel-vespa-bindings receive the staticized native link plan.
"""

from __future__ import annotations

import json
import os
import subprocess
import sys
from pathlib import Path


def output_path(args: list[str]) -> Path | None:
    for i, arg in enumerate(args[:-1]):
        if arg == "-o":
            return Path(args[i + 1])
    return None


def is_searchkernel_final_link(args: list[str]) -> bool:
    out = output_path(args)
    if out is None or out.suffix in {".so", ".dylib"}:
        return False
    return any("searchkernel_vespa_bindings" in arg for arg in args)


def main() -> int:
    args = sys.argv[1:]
    base = os.environ.get("STATIC_SK_BASE_LINKER", "cc")
    if not is_searchkernel_final_link(args):
        return subprocess.call([base, *args])

    plan_path = os.environ.get("STATIC_SK_LINK_ARGS_JSON")
    if not plan_path:
        print("STATIC_SK_LINK_ARGS_JSON is required for Candidate A", file=sys.stderr)
        return 2
    extra = json.loads(Path(plan_path).read_text(encoding="utf-8"))
    if not isinstance(extra, list) or not all(isinstance(x, str) for x in extra):
        print("invalid Candidate A link argument plan", file=sys.stderr)
        return 2

    cxx = os.environ.get("CXX", "c++")
    command = [cxx, *args, *extra]
    log_path = os.environ.get("STATIC_SK_FINAL_LINK_COMMAND")
    if log_path:
        Path(log_path).write_text(" ".join(command) + "\n", encoding="utf-8")
    return subprocess.call(command)


if __name__ == "__main__":
    sys.exit(main())
