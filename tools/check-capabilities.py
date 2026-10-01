#!/usr/bin/env python3
"""Validate the capability ledger and generate its agent-facing documents."""

from __future__ import annotations

import argparse
import json
import re
import sys
from pathlib import Path
from typing import Any


ROOT = Path(__file__).resolve().parents[1]
LEDGER_PATH = ROOT / "docs" / "capabilities.json"
RELEASE_SUPPORT_PATH = ROOT / "docs" / "release-support.json"
CAPABILITIES_DOC = ROOT / "docs" / "CAPABILITIES.md"
LIMITATIONS_DOC = ROOT / "LIMITATIONS.md"
QUERY_SCHEMA_PATH = ROOT / "api" / "searchkernel.fbs"
ID_PATTERN = re.compile(r"^[a-z][a-z0-9_.-]*\.[a-z0-9_.-]+$")
QUERY_EXPR_UNION_PATTERN = re.compile(
    r"\bunion\s+QueryExprKind\s*\{(?P<body>.*?)\}",
    re.DOTALL,
)
REQUIRED_FIELDS = {
    "id",
    "title",
    "area",
    "represented",
    "semantic",
    "executable",
    "verified_by",
    "limitations",
}
FACADE_SURFACES = {"rust", "python", "typescript"}
FACADE_TITLES = {"rust": "Rust", "python": "Python", "typescript": "TypeScript"}
FACADE_STATUSES = {"supported", "missing"}


class LedgerError(ValueError):
    """A user-facing ledger validation error."""


def load_ledger(path: Path = LEDGER_PATH) -> dict[str, Any]:
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except OSError as error:
        raise LedgerError(f"cannot read ledger {path}: {error}") from error
    except json.JSONDecodeError as error:
        raise LedgerError(f"invalid JSON in {path}: {error}") from error
    if not isinstance(value, dict):
        raise LedgerError("ledger root must be an object")
    return value



def load_release_support(path: Path = RELEASE_SUPPORT_PATH) -> dict[str, Any]:
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except OSError as error:
        raise LedgerError(f"cannot read release support {path}: {error}") from error
    except json.JSONDecodeError as error:
        raise LedgerError(f"invalid JSON in {path}: {error}") from error
    if not isinstance(value, dict):
        raise LedgerError("release support root must be an object")
    if value.get("version") != 1:
        raise LedgerError("release support version must be 1")
    _require_string(value.get("release"), "release support release")
    platform = value.get("platform")
    if not isinstance(platform, dict):
        raise LedgerError("release support platform must be an object")
    for key in ("os", "arch", "libc"):
        _require_string(platform.get(key), f"release support platform.{key}")
    supported = value.get("supported")
    if not isinstance(supported, list) or not supported:
        raise LedgerError("release support supported must be a non-empty list")
    for index, row in enumerate(supported):
        if not isinstance(row, dict):
            raise LedgerError(f"release support supported[{index}] must be an object")
        _require_string(row.get("surface"), f"release support supported[{index}].surface")
        _require_string(row.get("runtime"), f"release support supported[{index}].runtime")
    _require_string_list(value.get("deferred"), "release support deferred")
    return value


def _require_string(value: Any, label: str) -> str:
    if not isinstance(value, str) or not value.strip():
        raise LedgerError(f"{label} must be a non-empty string")
    return value


def _require_bool(value: Any, label: str) -> bool:
    if not isinstance(value, bool):
        raise LedgerError(f"{label} must be a boolean")
    return value


def _require_string_list(value: Any, label: str) -> list[str]:
    if not isinstance(value, list):
        raise LedgerError(f"{label} must be a list")
    result: list[str] = []
    for index, item in enumerate(value):
        result.append(_require_string(item, f"{label}[{index}]"))
    return result


def _source_has_symbol(source: str, symbol: str) -> bool:
    """Match declaration-shaped evidence, not comments or arbitrary substrings."""
    return any(line.strip().startswith(symbol) for line in source.splitlines())


def _validate_facade_support(
    value: Any,
    label: str,
    root: Path,
) -> dict[str, dict[str, Any]]:
    if not isinstance(value, dict):
        raise LedgerError(f"{label} must be an object")

    validated: dict[str, dict[str, Any]] = {}
    for surface, state in value.items():
        if surface not in FACADE_SURFACES:
            raise LedgerError(f"{label} has unknown facade surface: {surface}")
        if not isinstance(state, dict):
            raise LedgerError(f"{label}.{surface} must be an object")
        status = state.get("status")
        if status not in FACADE_STATUSES:
            raise LedgerError(
                f"{label}.{surface}.status must be one of: missing, supported"
            )
        raw_evidence = state.get("evidence", [])
        raw_absence_evidence = state.get("absence_evidence", [])
        if not isinstance(raw_evidence, list):
            raise LedgerError(f"{label}.{surface}.evidence must be a list")
        if not isinstance(raw_absence_evidence, list):
            raise LedgerError(f"{label}.{surface}.absence_evidence must be a list")
        if status == "supported" and not raw_evidence:
            raise LedgerError(
                f"{label}.{surface}: supported facade requires structural evidence"
            )
        if status == "supported" and raw_absence_evidence:
            raise LedgerError(
                f"{label}.{surface}: supported facade cannot list absence evidence"
            )
        if status == "missing" and raw_evidence:
            raise LedgerError(
                f"{label}.{surface} claims the facade is missing but lists "
                "implementation evidence"
            )
        if status == "missing" and not raw_absence_evidence:
            raise LedgerError(
                f"{label}.{surface}: missing facade requires absence evidence"
            )

        def validate_symbol_evidence(
            items: list[Any],
            evidence_name: str,
            expect_present: bool,
        ) -> list[dict[str, Any]]:
            validated_items: list[dict[str, Any]] = []
            for index, item in enumerate(items):
                evidence_label = f"{label}.{surface}.{evidence_name}[{index}]"
                if not isinstance(item, dict):
                    raise LedgerError(f"{evidence_label} must be an object")
                path_value = item.get("path")
                if not isinstance(path_value, str) or not path_value:
                    raise LedgerError(
                        f"{evidence_label}.path must be a non-empty string"
                    )
                path = Path(path_value)
                if path.is_absolute() or ".." in path.parts:
                    raise LedgerError(
                        f"{evidence_label}.path must be repository-relative: {path_value}"
                    )
                target = root / path
                if not target.is_file():
                    raise LedgerError(
                        f"{evidence_label}.path does not exist: {path_value}"
                    )
                symbols = _require_string_list(
                    item.get("symbols"), f"{evidence_label}.symbols"
                )
                try:
                    source = target.read_text(encoding="utf-8")
                except OSError as error:
                    raise LedgerError(
                        f"cannot read facade evidence {path_value}: {error}"
                    ) from error
                present = [symbol for symbol in symbols if _source_has_symbol(source, symbol)]
                if expect_present:
                    missing_symbols = [symbol for symbol in symbols if symbol not in present]
                    if missing_symbols:
                        raise LedgerError(
                            f"{label}.{surface} evidence is missing symbol(s): "
                            + ", ".join(missing_symbols)
                        )
                elif present:
                    raise LedgerError(
                        f"{label}.{surface} claims the facade is missing but "
                        "implementation symbol(s) now exist: "
                        + ", ".join(present)
                    )
                validated_items.append({"path": path.as_posix(), "symbols": symbols})
            return validated_items

        evidence = validate_symbol_evidence(raw_evidence, "evidence", True)
        absence_evidence = validate_symbol_evidence(
            raw_absence_evidence, "absence_evidence", False
        )
        validated[surface] = {
            "status": status,
            "evidence": evidence,
            "absence_evidence": absence_evidence,
        }
    return validated


def load_query_expr_kinds(path: Path = QUERY_SCHEMA_PATH) -> list[str]:
    try:
        schema = path.read_text(encoding="utf-8")
    except OSError as error:
        raise LedgerError(f"cannot read query schema {path}: {error}") from error
    match = QUERY_EXPR_UNION_PATTERN.search(schema)
    if match is None:
        raise LedgerError(f"cannot find QueryExprKind union in {path}")

    body = "\n".join(
        line.split("//", 1)[0] for line in match.group("body").splitlines()
    )
    kinds: list[str] = []
    for entry in body.split(","):
        entry = entry.strip()
        if not entry:
            continue
        if not re.fullmatch(r"[A-Za-z][A-Za-z0-9_]*", entry):
            raise LedgerError(f"invalid QueryExprKind entry in {path}: {entry}")
        kinds.append(entry)
    if not kinds:
        raise LedgerError(f"QueryExprKind union in {path} is empty")
    if len(kinds) != len(set(kinds)):
        raise LedgerError(f"duplicate QueryExprKind entry in {path}")
    return kinds


def validate_ledger(ledger: dict[str, Any], root: Path = ROOT) -> list[dict[str, Any]]:
    if ledger.get("version") != 1:
        raise LedgerError("ledger version must be 1")
    _require_string(ledger.get("snapshot"), "snapshot")
    capabilities = ledger.get("capabilities")
    if not isinstance(capabilities, list) or not capabilities:
        raise LedgerError("capabilities must be a non-empty list")

    # Read the union from the canonical schema so adding a typed-query variant
    # forces an explicit capability classification instead of silently passing
    # with an incomplete hand-maintained inventory.
    query_expr_kinds = set(load_query_expr_kinds(root / "api" / "searchkernel.fbs"))
    query_expr_owners: dict[str, str] = {}
    validated: list[dict[str, Any]] = []
    seen: set[str] = set()
    for index, capability in enumerate(capabilities):
        label = f"capabilities[{index}]"
        if not isinstance(capability, dict):
            raise LedgerError(f"{label} must be an object")
        missing = REQUIRED_FIELDS - capability.keys()
        if missing:
            raise LedgerError(f"{label} is missing: {', '.join(sorted(missing))}")
        identifier = _require_string(capability["id"], f"{label}.id")
        if not ID_PATTERN.fullmatch(identifier):
            raise LedgerError(f"{label}.id has invalid format: {identifier}")
        if identifier in seen:
            raise LedgerError(f"duplicate capability id: {identifier}")
        seen.add(identifier)
        _require_string(capability["title"], f"{label}.title")
        _require_string(capability["area"], f"{label}.area")

        if "query_expr_kinds" in capability:
            if capability["area"] != "query":
                raise LedgerError(
                    f"{identifier}: query_expr_kinds is only valid for query capabilities"
                )
            kinds = _require_string_list(
                capability.get("query_expr_kinds"),
                f"{label}.query_expr_kinds",
            )
            for kind in kinds:
                if kind not in query_expr_kinds:
                    raise LedgerError(
                        f"{identifier}: unknown QueryExprKind: {kind}"
                    )
                previous = query_expr_owners.get(kind)
                if previous is not None:
                    raise LedgerError(
                        f"{kind} is assigned to both {previous} and {identifier}"
                    )
                query_expr_owners[kind] = identifier

        represented = _require_bool(capability["represented"], f"{label}.represented")
        semantic = _require_bool(capability["semantic"], f"{label}.semantic")
        executable = _require_bool(capability["executable"], f"{label}.executable")
        if semantic and not represented:
            raise LedgerError(f"{identifier}: semantic support requires representation")
        if executable and not semantic:
            raise LedgerError(f"{identifier}: executable support requires semantic support")

        verified_by = _require_string_list(
            capability["verified_by"], f"{label}.verified_by"
        )
        if executable and not verified_by:
            raise LedgerError(f"{identifier}: executable support requires evidence")
        for reference in verified_by:
            reference_path = Path(reference)
            if reference_path.is_absolute() or ".." in reference_path.parts:
                raise LedgerError(
                    f"{identifier}: evidence path must be repository-relative: {reference}"
                )
            if not (root / reference_path).is_file():
                raise LedgerError(
                    f"{identifier}: evidence path does not exist: {reference}"
                )

        limitations = _require_string_list(capability["limitations"], f"{label}.limitations")
        if not represented and not limitations:
            raise LedgerError(f"{identifier}: unsupported capability needs a limitation")

        if "facade_support" in capability:
            _validate_facade_support(
                capability["facade_support"], f"{label}.facade_support", root
            )

        validated.append(capability)

    missing_query_expr_kinds = query_expr_kinds - query_expr_owners.keys()
    if missing_query_expr_kinds:
        missing = ", ".join(sorted(missing_query_expr_kinds))
        raise LedgerError(f"unclassified QueryExprKind variants: {missing}")
    return validated


def _markdown_text(value: str) -> str:
    return value.replace("|", "\\|").replace("\n", " ")


def _link(path: str, from_directory: Path) -> str:
    target = Path(path)
    relative = Path("..") / target if from_directory != ROOT else target
    return f"[{path}]({relative.as_posix()})"


def _status(value: bool) -> str:
    return "yes" if value else "no"


def render_capabilities(ledger: dict[str, Any], capabilities: list[dict[str, Any]]) -> str:
    lines = [
        "# Capability ledger",
        "",
        "This file is generated from [`capabilities.json`](capabilities.json).",
        "It is the current cross-layer implementation view; contracts and verifiers remain",
        "the authority for required behavior.",
        "",
        "Per-client/runtime support is generated from executable evidence in",
        "[`CLIENT_CAPABILITIES.md`](CLIENT_CAPABILITIES.md), not from this ledger.",
        "",
        f"**Snapshot:** {ledger['snapshot']}",
        "",
        "The state columns mean: represented in the portable contract, resolved by",
        "canonical semantics, and executable through the backend.",
        "",
        "| Capability | Area | Represented | Semantic | Executable | Evidence |",
        "| --- | --- | ---: | ---: | ---: | --- |",
    ]
    for capability in capabilities:
        evidence = ", ".join(
            _link(path, ROOT / "docs") for path in capability["verified_by"]
        ) or "—"
        lines.append(
            "| {title} (`{identifier}`) | {area} | {represented} | {semantic} | "
            "{executable} | {evidence} |".format(
                title=_markdown_text(capability["title"]),
                identifier=capability["id"],
                area=_markdown_text(capability["area"]),
                represented=_status(capability["represented"]),
                semantic=_status(capability["semantic"]),
                executable=_status(capability["executable"]),
                evidence=evidence,
            )
        )
    lines.extend(
        [
            "",
            "## Limitations at a glance",
            "",
            "Capabilities with known constraints are collected in the generated",
            "[`LIMITATIONS.md`](../LIMITATIONS.md).",
            "",
        ]
    )
    for capability in capabilities:
        if capability["limitations"]:
            lines.append(f"- `{capability['id']}` — {capability['limitations'][0]}")
    structured = [
        capability for capability in capabilities if "facade_support" in capability
    ]
    if structured:
        lines.extend(["", "## Structured facade support", ""])
        lines.extend(
            [
                "Facade availability below is validated against implementation symbols "
                "and executable client evidence.",
                "",
                "| Capability | Surface | Status | Structural evidence |",
                "| --- | --- | --- | --- |",
            ]
        )
        for capability in structured:
            for surface, state in capability["facade_support"].items():
                evidence = ", ".join(item["path"] for item in state["evidence"]) or "—"
                lines.append(
                    f"| `{capability['id']}` | {surface} | {state['status']} | {evidence} |"
                )
    lines.append("")
    return "\n".join(lines)


def render_limitations(
    ledger: dict[str, Any],
    capabilities: list[dict[str, Any]],
    release_support: dict[str, Any],
) -> str:
    lines = [
        "# Search Kernel limitations",
        "",
        "This file is generated from [`docs/capabilities.json`](docs/capabilities.json).",
        "Search Kernel is a thin wrapper around the pinned Vespa engine, but its",
        "public contract is intentionally smaller than Vespa's complete model.",
        "Unsupported behavior must remain an explicit error; it must not be silently",
        "approximated.",
        "",
        f"**Snapshot:** {ledger['snapshot']}",
        "",
        "## Release scope",
        "",
        f"- The `{release_support['release']}` release is qualified only for "
        f"**{release_support['platform']['os']} {release_support['platform']['arch']} "
        f"{release_support['platform']['libc']}**.",
        "- Supported release surfaces: "
        + ", ".join(
            f"`{row['surface']}` (`{row['runtime']}`)"
            for row in release_support["supported"]
        )
        + ".",
        "- Deferred release surfaces: "
        + ", ".join(f"`{surface}`" for surface in release_support["deferred"])
        + ".",
        "- Implementation presence outside this matrix is not a release-support promise; "
        "see [`docs/release-support.md`](docs/release-support.md).",
        "",
    ]
    by_area: dict[str, list[dict[str, Any]]] = {}
    for capability in capabilities:
        missing_facades = [
            surface
            for surface, state in capability.get("facade_support", {}).items()
            if state["status"] == "missing"
        ]
        if capability["limitations"] or missing_facades:
            by_area.setdefault(capability["area"], []).append(capability)
    for area, entries in by_area.items():
        lines.extend([f"## {area.title()}", ""])
        for capability in entries:
            lines.append(f"### {capability['title']} (`{capability['id']}`)")
            lines.append("")
            for surface, state in capability.get("facade_support", {}).items():
                if state["status"] == "missing":
                    lines.append(
                        f"- The {FACADE_TITLES[surface]} facade does not expose this capability "
                        "(machine-checked structured support state)."
                    )
            for limitation in capability["limitations"]:
                lines.append(f"- {_markdown_text(limitation)}")
            lines.append("")
    lines.extend(
        [
            "## Evidence and versioning",
            "",
            "The state and limitations are cross-layer summaries. Behavioral authority",
            "remains the frozen contract, pinned Vespa source, direct oracle, and",
            "acceptance verifier in that order. Vespa behavior is pinned to the revision",
            "recorded in [`reference/vespa/PINNED_VESPA.json`](reference/vespa/PINNED_VESPA.json).",
            "",
        ]
    )
    return "\n".join(lines)


def generated_documents(ledger: dict[str, Any]) -> dict[Path, str]:
    capabilities = validate_ledger(ledger)
    release_support = load_release_support()
    return {
        CAPABILITIES_DOC: render_capabilities(ledger, capabilities),
        LIMITATIONS_DOC: render_limitations(ledger, capabilities, release_support),
    }


def check_documents(documents: dict[Path, str]) -> list[str]:
    stale: list[str] = []
    for path, expected in documents.items():
        actual = path.read_text(encoding="utf-8") if path.is_file() else None
        if actual != expected:
            try:
                display_path = str(path.relative_to(ROOT))
            except ValueError:
                display_path = str(path)
            stale.append(display_path)
    return stale


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    mode = parser.add_mutually_exclusive_group()
    mode.add_argument("--check", action="store_true", help="fail if generated docs are stale")
    mode.add_argument("--write", action="store_true", help="write generated docs")
    args = parser.parse_args(argv)

    try:
        ledger = load_ledger()
        documents = generated_documents(ledger)
        stale = check_documents(documents)
        if args.write:
            for path, content in documents.items():
                path.write_text(content, encoding="utf-8")
            print("Capability documents updated.")
            return 0
        if stale:
            print("Capability documents are stale:", file=sys.stderr)
            for path in stale:
                print(f"- {path}", file=sys.stderr)
            print("Run: python3 tools/check-capabilities.py --write", file=sys.stderr)
            return 1
        print("Capability ledger and generated documents PASS.")
        return 0
    except (LedgerError, OSError) as error:
        print(f"Capability check FAILED: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
