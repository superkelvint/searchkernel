#!/usr/bin/env python3
"""Reject CI workflows that mutate the shared Rust toolchain on self-hosted runners."""

from __future__ import annotations

import re
import sys
from pathlib import Path

WORKFLOWS = Path(".github/workflows")

FORBIDDEN = (
    (re.compile(r"dtolnay/rust-toolchain@"), "job-local Rust toolchain installation"),
    (re.compile(r"\brustup\s+toolchain\s+(?:install|uninstall|remove)\b"), "rustup toolchain mutation"),
    (re.compile(r"\brustup\s+default\b"), "rustup default mutation"),
    (re.compile(r"\brustup\s+update\b"), "rustup update"),
    (re.compile(r"\brustup\s+self\s+update\b"), "rustup self update"),
    (re.compile(r"\brustup\s+component\s+(?:add|remove)\b"), "rustup component mutation"),
    (re.compile(r"sh\.rustup\.rs"), "rustup bootstrap"),
)


def action_block(lines: list[str], start: int) -> list[str]:
    indent = len(lines[start]) - len(lines[start].lstrip())
    block = [lines[start]]
    for line in lines[start + 1 :]:
        stripped = line.lstrip()
        current = len(line) - len(stripped)
        if stripped.startswith("- ") and current <= indent:
            break
        block.append(line)
    return block


def main() -> int:
    failures: list[str] = []

    for path in sorted(WORKFLOWS.glob("*.y*ml")):
        text = path.read_text(encoding="utf-8")
        lines = text.splitlines()

        # All SearchKernel Actions jobs currently run on persistent self-hosted
        # runners. Rust is runner provisioning, not job-local mutable state.
        if "self-hosted" in text:
            for pattern, description in FORBIDDEN:
                for match in pattern.finditer(text):
                    line = text.count("\n", 0, match.start()) + 1
                    failures.append(f"{path}:{line}: forbidden {description}")

        for index, line in enumerate(lines):
            if "uses: Swatinem/rust-cache@" not in line:
                continue
            block = "\n".join(action_block(lines, index))
            if not re.search(r"(?m)^\s*cache-bin:\s*(?:false|['\"]false['\"])\s*$", block):
                failures.append(
                    f"{path}:{index + 1}: Swatinem/rust-cache on persistent runners "
                    "must set cache-bin: false"
                )

    if failures:
        print("Self-hosted Rust workflow policy violations:", file=sys.stderr)
        for failure in failures:
            print(f"  - {failure}", file=sys.stderr)
        print(
            "\nRust toolchains are provisioned once on the runner. CI jobs may "
            "verify cargo/rustc/rustfmt/clippy, but must never install, update, "
            "remove, select, or cache toolchain binaries.",
            file=sys.stderr,
        )
        return 1

    print("Self-hosted Rust workflow policy: PASS")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
