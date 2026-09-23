#!/usr/bin/env python3
"""Verify the public Search Kernel C ABI contract."""

from __future__ import annotations

import argparse
import os
import re
import sys
from pathlib import Path


# An entry is intentionally a string rather than a boolean.  Every exception
# must carry the benchmark and review reference that justified the second ABI
# layout contract.
ALLOWLIST: dict[str, str] = {
    # "sk_text_occurrence_batch_v2":
    #     "bench/occurrence_ingest.rs shows 3.1x throughput gain, PR #482",
}

_ABI_VERSION_RE = re.compile(
    r"^\s*#\s*define\s+SK_NATIVE_ABI_VERSION\s+(\d+)U?\s*$",
    re.MULTILINE,
)
_LINKER_SYMBOL_RE = re.compile(r"^\s*(sk_[A-Za-z0-9_]+)\s*;\s*$", re.MULTILINE)


def _load_clang():
    try:
        import clang.cindex as ci
    except ImportError as error:
        raise RuntimeError(
            "libclang's Python bindings are required; install the "
            "python3-clang and libclang packages"
        ) from error
    return ci


def _parse_header(header_path: str | os.PathLike[str]):
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
    return ci, path, translation_unit


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


def _canonical_type_spelling(type_) -> str:
    return " ".join(type_.get_canonical().spelling.split())


def function_signatures(
    header_path: str | os.PathLike[str],
) -> dict[str, str]:
    """Return normalized libclang signatures for public header functions."""

    ci, path, translation_unit = _parse_header(header_path)
    signatures: dict[str, str] = {}
    for cursor in _header_function_declarations(translation_unit, path, ci):
        name = cursor.spelling
        if not name.startswith("sk_"):
            continue
        arguments = ", ".join(
            _canonical_type_spelling(argument.type)
            for argument in cursor.get_arguments()
        )
        signature = f"{_canonical_type_spelling(cursor.result_type)} ({arguments})"
        previous = signatures.get(name)
        if previous is not None and previous != signature:
            raise RuntimeError(
                f"ABI header declares {name} with multiple signatures: "
                f"{previous!r} and {signature!r}"
            )
        signatures[name] = signature
    return signatures


def read_abi_version(header_path: str | os.PathLike[str]) -> int:
    path = Path(header_path).resolve()
    try:
        contents = path.read_text(encoding="utf-8")
    except OSError as error:
        raise RuntimeError(f"could not read ABI header {path}: {error}") from error
    matches = _ABI_VERSION_RE.findall(contents)
    if len(matches) != 1:
        raise RuntimeError(
            f"ABI header {path} must define SK_NATIVE_ABI_VERSION exactly once"
        )
    return int(matches[0])


def linker_map_symbols(linker_map_path: str | os.PathLike[str]) -> set[str]:
    path = Path(linker_map_path).resolve()
    if not path.is_file():
        raise RuntimeError(f"linker map does not exist: {path}")
    try:
        contents = path.read_text(encoding="utf-8")
    except OSError as error:
        raise RuntimeError(f"could not read linker map {path}: {error}") from error
    return set(_LINKER_SYMBOL_RE.findall(contents))


def check(header_path: str | os.PathLike[str]) -> list[str]:
    """Return ABI aggregate violations found in a public C header."""

    ci, path, translation_unit = _parse_header(header_path)

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


def _signature_drift(
    baseline: dict[str, str],
    current: dict[str, str],
) -> list[str]:
    drift: list[str] = []
    for name in sorted(baseline.keys() - current.keys()):
        drift.append(f"removed {name}: {baseline[name]}")
    for name in sorted(current.keys() - baseline.keys()):
        drift.append(f"added {name}: {current[name]}")
    for name in sorted(baseline.keys() & current.keys()):
        if baseline[name] != current[name]:
            drift.append(f"changed {name}: {baseline[name]} -> {current[name]}")
    return drift


def check_contract(
    header_path: str | os.PathLike[str],
    *,
    linker_map_path: str | os.PathLike[str] | None = None,
    baseline_header_path: str | os.PathLike[str] | None = None,
) -> list[str]:
    """Check aggregate safety, export parity, and ABI-version transitions."""

    violations = check(header_path)
    current_signatures = function_signatures(header_path)

    if linker_map_path is not None:
        exported = linker_map_symbols(linker_map_path)
        declared = set(current_signatures)
        for name in sorted(declared - exported):
            violations.append(
                f"{name}: declared in header but missing from linker map"
            )
        for name in sorted(exported - declared):
            violations.append(
                f"{name}: exported by linker map but missing from header"
            )

    if baseline_header_path is not None:
        baseline_signatures = function_signatures(baseline_header_path)
        drift = _signature_drift(baseline_signatures, current_signatures)
        if drift:
            baseline_version = read_abi_version(baseline_header_path)
            current_version = read_abi_version(header_path)
            if current_version <= baseline_version:
                preview = "; ".join(drift[:8])
                if len(drift) > 8:
                    preview += f"; ... {len(drift) - 8} more"
                violations.append(
                    "native ABI signature surface changed without increasing "
                    "SK_NATIVE_ABI_VERSION "
                    f"({baseline_version} -> {current_version}): {preview}"
                )

    return violations


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        description="check the public Search Kernel C ABI contract"
    )
    parser.add_argument("header", type=Path)
    parser.add_argument("--linker-map", type=Path)
    parser.add_argument("--baseline-header", type=Path)
    args = parser.parse_args(argv)

    try:
        violations = check_contract(
            args.header,
            linker_map_path=args.linker_map,
            baseline_header_path=args.baseline_header,
        )
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
