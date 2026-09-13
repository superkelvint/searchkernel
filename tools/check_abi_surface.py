#!/usr/bin/env python3
"""Reject defined C aggregates in the public Search Kernel ABI."""

from __future__ import annotations

import argparse
import os
import sys
from pathlib import Path


# An entry is intentionally a string rather than a boolean.  Every exception
# must carry the benchmark and review reference that justified the second ABI
# layout contract.
ALLOWLIST: dict[str, str] = {
    # "sk_text_occurrence_batch_v2":
    #     "bench/occurrence_ingest.rs shows 3.1x throughput gain, PR #482",
}


def _load_clang():
    try:
        import clang.cindex as ci
    except ImportError as error:
        raise RuntimeError(
            "libclang's Python bindings are required; install the "
            "python3-clang and libclang packages"
        ) from error
    return ci


def pointee_is_defined_aggregate(type_, ci_module=None) -> str | None:
    """Return a defined record name, stripping one pointer indirection."""

    ci = ci_module or _load_clang()
    type_ = type_.get_canonical()
    if type_.kind == ci.TypeKind.POINTER:
        type_ = type_.get_pointee()
    type_ = type_.get_canonical()
    if type_.kind == ci.TypeKind.RECORD:
        declaration = type_.get_declaration()
        if declaration.is_definition():
            return declaration.spelling or type_.spelling
    return None


def _header_function_declarations(translation_unit, header_path: Path, ci):
    """Yield function declarations physically written in *header_path*."""

    header_path = header_path.resolve()
    for cursor in translation_unit.cursor.get_children():
        if cursor.kind != ci.CursorKind.FUNCTION_DECL:
            continue
        if cursor.location.file is None:
            continue
        try:
            in_header = Path(cursor.location.file.name).resolve() == header_path
        except OSError:
            in_header = False
        if in_header:
            yield cursor


def check(header_path: str | os.PathLike[str]) -> list[str]:
    """Return ABI aggregate violations found in a public C header."""

    ci = _load_clang()
    path = Path(header_path).resolve()
    if not path.is_file():
        raise RuntimeError(f"ABI header does not exist: {path}")

    index = ci.Index.create()
    translation_unit = index.parse(str(path), args=["-x", "c", "-std=c11"])
    diagnostics = [
        str(diagnostic)
        for diagnostic in translation_unit.diagnostics
        if diagnostic.severity >= ci.Diagnostic.Error
    ]
    if diagnostics:
        raise RuntimeError(
            f"libclang could not parse {path}:\n" + "\n".join(diagnostics)
        )

    violations: list[str] = []
    for cursor in _header_function_declarations(translation_unit, path, ci):
        justification = ALLOWLIST.get(cursor.spelling)
        for argument in cursor.get_arguments():
            name = pointee_is_defined_aggregate(argument.type, ci)
            if name and justification is None:
                transport = (
                    "by pointer"
                    if argument.type.get_canonical().kind == ci.TypeKind.POINTER
                    else "by value"
                )
                violations.append(
                    f"{cursor.spelling}: exposes defined struct '{name}' "
                    f"{transport}, not opaque, no allowlist entry"
                )

        return_name = pointee_is_defined_aggregate(cursor.result_type, ci)
        if return_name and justification is None:
            violations.append(
                f"{cursor.spelling}: returns defined struct '{return_name}' "
                "by value/pointer"
            )

    # A blank allowlist value would turn the exception into an unreviewed
    # boolean. Report it even if the function currently has no aggregate use.
    for function_name, justification in ALLOWLIST.items():
        if not justification.strip():
            violations.append(
                f"{function_name}: allowlist entry requires a benchmark and "
                "linked review justification"
            )

    return violations


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        description="check a public C ABI header for defined aggregate types"
    )
    parser.add_argument("header", type=Path)
    args = parser.parse_args(argv)

    try:
        violations = check(args.header)
    except RuntimeError as error:
        print(f"ABI surface check: {error}", file=sys.stderr)
        return 2

    if violations:
        print("ABI surface check FAILED:", file=sys.stderr)
        for violation in violations:
            print(f"- {violation}", file=sys.stderr)
        return 1

    print(f"ABI surface check PASS: {args.header}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
