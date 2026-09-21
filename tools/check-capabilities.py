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
CAPABILITIES_DOC = ROOT / "docs" / "CAPABILITIES.md"
LIMITATIONS_DOC = ROOT / "LIMITATIONS.md"
ID_PATTERN = re.compile(r"^[a-z][a-z0-9_.-]*\.[a-z0-9_.-]+$")
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


def validate_ledger(ledger: dict[str, Any], root: Path = ROOT) -> list[dict[str, Any]]:
    if ledger.get("version") != 1:
        raise LedgerError("ledger version must be 1")
    _require_string(ledger.get("snapshot"), "snapshot")
    capabilities = ledger.get("capabilities")
    if not isinstance(capabilities, list) or not capabilities:
        raise LedgerError("capabilities must be a non-empty list")

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

        validated.append(capability)
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
    lines.append("")
    return "\n".join(lines)


def render_limitations(ledger: dict[str, Any], capabilities: list[dict[str, Any]]) -> str:
    lines = [
        "# Vespa compatibility limitations",
        "",
        "This file is generated from [`docs/capabilities.json`](docs/capabilities.json).",
        "Search Kernel is a thin wrapper around the pinned Vespa engine, but its",
        "public contract is intentionally smaller than Vespa's complete model.",
        "Unsupported behavior must remain an explicit error; it must not be silently",
        "approximated.",
        "",
        f"**Snapshot:** {ledger['snapshot']}",
        "",
    ]
    by_area: dict[str, list[dict[str, Any]]] = {}
    for capability in capabilities:
        if capability["limitations"]:
            by_area.setdefault(capability["area"], []).append(capability)
    for area, entries in by_area.items():
        lines.extend([f"## {area.title()}", ""])
        for capability in entries:
            lines.append(f"### {capability['title']} (`{capability['id']}`)")
            lines.append("")
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
    return {
        CAPABILITIES_DOC: render_capabilities(ledger, capabilities),
        LIMITATIONS_DOC: render_limitations(ledger, capabilities),
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
