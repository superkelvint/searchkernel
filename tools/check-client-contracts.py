#!/usr/bin/env python3
"""Validate executable client capability evidence and generate compatibility reports."""

from __future__ import annotations

import argparse
import json
import re
import subprocess
import sys
from collections import defaultdict
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[1]
CATALOG_PATH = ROOT / "reference" / "client-contracts" / "catalog.json"
GLOBAL_LEDGER_PATH = ROOT / "docs" / "capabilities.json"
REPORT_JSON = ROOT / "docs" / "client-capabilities.json"
REPORT_MD = ROOT / "docs" / "CLIENT_CAPABILITIES.md"
VALID_KINDS = {"ergonomic", "conformance", "e2e"}
SOURCE_SUFFIXES = {
    ".rs", ".py", ".php", ".rb", ".ts", ".tsx", ".js", ".mjs", ".c", ".cc", ".cpp", ".h", ".sh"
}
ANNOTATION_RE = re.compile(
    r"SKC:\s*(?P<capability>[a-z][a-z0-9_.-]*\.[a-z0-9_.-]+)/"
    r"(?P<scenario>[a-z0-9_.-]+)\s+"
    r"kinds=(?P<kinds>[a-z0-9_,-]+)\s+"
    r"runtime=(?P<runtime>[a-z0-9_.-]+)\s*$"
)


class RegistryError(ValueError):
    pass


def load_global_capabilities(path: Path = GLOBAL_LEDGER_PATH) -> dict[str, dict[str, Any]]:
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        raise RegistryError(f"cannot read global capability ledger {path}: {error}") from error
    items = value.get("capabilities") if isinstance(value, dict) else None
    if not isinstance(items, list):
        raise RegistryError("global capability ledger must contain a capabilities list")
    result: dict[str, dict[str, Any]] = {}
    for item in items:
        if not isinstance(item, dict) or not isinstance(item.get("id"), str):
            raise RegistryError("global capability entries must have string ids")
        if item["id"] in result:
            raise RegistryError(f"duplicate global capability id: {item['id']}")
        result[item["id"]] = item
    return result


def load_catalog(path: Path = CATALOG_PATH) -> dict[str, Any]:
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        raise RegistryError(f"cannot read client contract catalog {path}: {error}") from error
    if not isinstance(value, dict):
        raise RegistryError("client contract catalog root must be an object")
    return value

def validate_catalog(
    catalog: dict[str, Any],
    global_capabilities: dict[str, dict[str, Any]],
    root: Path = ROOT,
) -> tuple[dict[str, dict[str, Any]], dict[str, dict[str, str]], list[dict[str, Any]]]:
    if catalog.get("version") != 1:
        raise RegistryError("client contract catalog version must be 1")

    raw_surfaces = catalog.get("surfaces")
    if not isinstance(raw_surfaces, dict) or not raw_surfaces:
        raise RegistryError("catalog must define at least one surface")

    surfaces: dict[str, dict[str, Any]] = {}
    for name, config in raw_surfaces.items():
        if not isinstance(name, str) or not re.fullmatch(r"[a-z][a-z0-9_-]*", name):
            raise RegistryError(f"invalid surface name: {name!r}")
        if not isinstance(config, dict):
            raise RegistryError(f"surface {name} must be a table")
        title = config.get("title")
        runtimes = config.get("runtimes")
        roots = config.get("evidence_roots")
        raw_lanes = config.get("e2e_lanes", [])
        required_e2e_runtimes = config.get("required_e2e_runtimes", [])
        if not isinstance(title, str) or not title.strip():
            raise RegistryError(f"surface {name}.title must be a non-empty string")
        if not isinstance(runtimes, list) or not runtimes or not all(
            isinstance(item, str) and item for item in runtimes
        ):
            raise RegistryError(f"surface {name}.runtimes must be a non-empty string list")
        if len(set(runtimes)) != len(runtimes):
            raise RegistryError(f"surface {name}.runtimes contains duplicates")
        if not isinstance(roots, list) or not roots or not all(
            isinstance(item, str) and item for item in roots
        ):
            raise RegistryError(f"surface {name}.evidence_roots must be a non-empty string list")
        evidence_paths = []
        for evidence_root in roots:
            path = Path(evidence_root)
            if path.is_absolute() or ".." in path.parts:
                raise RegistryError(f"surface {name} has invalid evidence root: {evidence_root}")
            if not (root / path).exists():
                raise RegistryError(f"surface {name} evidence root does not exist: {evidence_root}")
            evidence_paths.append(path)
        if not isinstance(raw_lanes, list):
            raise RegistryError(f"surface {name}.e2e_lanes must be a list")
        if not isinstance(required_e2e_runtimes, list) or not all(
            isinstance(item, str) and item in runtimes for item in required_e2e_runtimes
        ):
            raise RegistryError(
                f"surface {name}.required_e2e_runtimes must be a list of surface runtimes"
            )
        if len(set(required_e2e_runtimes)) != len(required_e2e_runtimes):
            raise RegistryError(f"surface {name}.required_e2e_runtimes contains duplicates")
        e2e_lanes: list[dict[str, Any]] = []
        for lane_index, lane in enumerate(raw_lanes):
            label = f"surface {name}.e2e_lanes[{lane_index}]"
            if not isinstance(lane, dict):
                raise RegistryError(f"{label} must be a table")
            lane_runtimes = lane.get("runtimes")
            lane_path = lane.get("path")
            command = lane.get("command")
            if not isinstance(lane_runtimes, list) or not lane_runtimes or not all(
                isinstance(item, str) and item in runtimes for item in lane_runtimes
            ):
                raise RegistryError(f"{label}.runtimes must be a non-empty subset of surface runtimes")
            if len(set(lane_runtimes)) != len(lane_runtimes):
                raise RegistryError(f"{label}.runtimes contains duplicates")
            if not isinstance(lane_path, str) or not lane_path:
                raise RegistryError(f"{label}.path must be a non-empty string")
            relative_path = Path(lane_path)
            if relative_path.is_absolute() or ".." in relative_path.parts:
                raise RegistryError(f"{label}.path must be repository-relative")
            if not any(
                relative_path == evidence_path or evidence_path in relative_path.parents
                for evidence_path in evidence_paths
            ):
                raise RegistryError(f"{label}.path must be inside an evidence root")
            if not (root / relative_path).is_file():
                raise RegistryError(f"{label}.path does not exist: {lane_path}")
            if not isinstance(command, list) or not command or not all(
                isinstance(item, str) and item for item in command
            ):
                raise RegistryError(f"{label}.command must be a non-empty string list")
            e2e_lanes.append(
                {
                    "runtimes": list(lane_runtimes),
                    "path": relative_path.as_posix(),
                    "command": list(command),
                }
            )
        for runtime in required_e2e_runtimes:
            if not any(runtime in lane["runtimes"] for lane in e2e_lanes):
                raise RegistryError(
                    f"surface {name} requires an E2E lane for runtime {runtime}"
                )
        surfaces[name] = {
            "title": title,
            "runtimes": runtimes,
            "evidence_roots": roots,
            "e2e_lanes": e2e_lanes,
            "required_e2e_runtimes": list(required_e2e_runtimes),
        }

    scenarios = {
        capability_id: {"basic": "Basic behavior"}
        for capability_id in global_capabilities
    }
    for index, item in enumerate(catalog.get("scenarios", [])):
        if not isinstance(item, dict):
            raise RegistryError(f"scenarios[{index}] must be a table")
        capability = item.get("capability")
        scenario = item.get("id")
        title = item.get("title")
        if capability not in global_capabilities:
            raise RegistryError(f"unknown scenario capability: {capability}")
        if not isinstance(scenario, str) or not re.fullmatch(r"[a-z0-9][a-z0-9_.-]*", scenario):
            raise RegistryError(f"invalid scenario id for {capability}: {scenario!r}")
        if not isinstance(title, str) or not title.strip():
            raise RegistryError(f"scenario {capability}/{scenario} needs a title")
        scenarios[capability][scenario] = title

    raw_overrides = catalog.get("overrides", [])
    if not isinstance(raw_overrides, list):
        raise RegistryError("overrides must be an array of tables")
    overrides: list[dict[str, Any]] = []
    for index, item in enumerate(raw_overrides):
        if not isinstance(item, dict):
            raise RegistryError(f"overrides[{index}] must be a table")
        capability = item.get("capability")
        scenario = item.get("scenario", "basic")
        surface = item.get("surface")
        runtime = item.get("runtime")
        status = item.get("status")
        reason = item.get("reason")
        if capability not in scenarios or scenario not in scenarios[capability]:
            raise RegistryError(f"override references unknown contract: {capability}/{scenario}")
        if surface not in surfaces:
            raise RegistryError(f"override references unknown surface: {surface}")
        if runtime not in surfaces[surface]["runtimes"]:
            raise RegistryError(f"override references unknown runtime for {surface}: {runtime}")
        if status not in {"blocked", "not-applicable"}:
            raise RegistryError(f"override status must be blocked or not-applicable: {status}")
        if not isinstance(reason, str) or not reason.strip():
            raise RegistryError(f"override {capability}/{scenario} needs a reason")
        if status == "blocked" and not isinstance(item.get("issue"), int):
            raise RegistryError(f"blocked override {capability}/{scenario} needs an issue number")
        overrides.append(dict(item))
    return surfaces, scenarios, overrides


def collect_evidence(
    surfaces: dict[str, dict[str, Any]],
    scenarios: dict[str, dict[str, str]],
    root: Path = ROOT,
) -> list[dict[str, Any]]:
    evidence: list[dict[str, Any]] = []
    for surface, config in surfaces.items():
        allowed_runtimes = set(config["runtimes"])
        for evidence_root in config["evidence_roots"]:
            base = root / evidence_root
            paths = [base] if base.is_file() else sorted(
                path for path in base.rglob("*") if path.is_file()
            )
            for path in paths:
                if path.suffix not in SOURCE_SUFFIXES:
                    continue
                try:
                    lines = path.read_text(encoding="utf-8").splitlines()
                except UnicodeDecodeError as error:
                    raise RegistryError(f"cannot decode evidence file {path}: {error}") from error
                for line_number, line in enumerate(lines, 1):
                    if "SKC:" not in line:
                        continue
                    match = ANNOTATION_RE.search(line)
                    relative = path.relative_to(root).as_posix()
                    if match is None:
                        raise RegistryError(
                            f"malformed SKC annotation at {relative}:{line_number}: {line.strip()}"
                        )
                    capability = match.group("capability")
                    scenario = match.group("scenario")
                    runtime = match.group("runtime")
                    kinds = match.group("kinds").split(",")
                    if capability not in scenarios:
                        raise RegistryError(
                            f"unknown capability in evidence: {capability} at {relative}:{line_number}"
                        )
                    if scenario not in scenarios[capability]:
                        raise RegistryError(
                            f"unknown scenario in evidence: {capability}/{scenario} at {relative}:{line_number}"
                        )
                    if runtime not in allowed_runtimes:
                        raise RegistryError(
                            f"unknown runtime {runtime} for {surface} at {relative}:{line_number}"
                        )
                    if not kinds or any(kind not in VALID_KINDS for kind in kinds) or len(set(kinds)) != len(kinds):
                        raise RegistryError(
                            f"invalid evidence kinds at {relative}:{line_number}: {match.group('kinds')}"
                        )
                    if "e2e" in kinds and not any(
                        lane["path"] == relative and runtime in lane["runtimes"]
                        for lane in config["e2e_lanes"]
                    ):
                        raise RegistryError(
                            f"E2E evidence at {relative}:{line_number} is not covered by an E2E CI lane"
                        )
                    for kind in kinds:
                        evidence.append(
                            {
                                "capability": capability,
                                "scenario": scenario,
                                "surface": surface,
                                "runtime": runtime,
                                "kind": kind,
                                "path": relative,
                                "line": line_number,
                            }
                        )
    return sorted(
        evidence,
        key=lambda item: (
            item["capability"],
            item["scenario"],
            item["surface"],
            item["kind"],
            item["runtime"],
            item["path"],
            item["line"],
        ),
    )


def validate_facade_support(
    global_capabilities: dict[str, dict[str, Any]],
    surfaces: dict[str, dict[str, Any]],
    evidence: list[dict[str, Any]],
) -> None:
    """Ensure structured facade claims agree with executable ergonomic evidence."""
    ergonomic_support = {
        (item["capability"], item["surface"])
        for item in evidence
        if item["kind"] == "ergonomic"
    }
    for capability_id, capability in global_capabilities.items():
        raw_support = capability.get("facade_support", {})
        if not isinstance(raw_support, dict):
            raise RegistryError(f"{capability_id}.facade_support must be an object")
        for surface, state in raw_support.items():
            if surface not in surfaces:
                raise RegistryError(
                    f"{capability_id}.facade_support references unknown surface: {surface}"
                )
            if not isinstance(state, dict) or state.get("status") not in {
                "supported",
                "missing",
            }:
                raise RegistryError(
                    f"{capability_id}.facade_support.{surface}.status must be "
                    "supported or missing"
                )
            has_evidence = (capability_id, surface) in ergonomic_support
            status = state["status"]
            if status == "supported" and not has_evidence:
                raise RegistryError(
                    f"{capability_id} claims {surface} is supported but has no ergonomic evidence"
                )
            if status == "missing" and has_evidence:
                raise RegistryError(
                    f"{capability_id} claims {surface} is missing but executable "
                    "evidence registers support"
                )


def run_e2e_lanes(
    surfaces: dict[str, dict[str, Any]],
    root: Path = ROOT,
    surface: str | None = None,
) -> None:
    if surface is not None:
        if surface not in surfaces:
            raise RegistryError(f"unknown E2E client surface: {surface}")
        selected_surfaces = ((surface, surfaces[surface]),)
    else:
        selected_surfaces = surfaces.items()

    seen: set[tuple[str, ...]] = set()
    for surface_name, config in selected_surfaces:
        for lane in config["e2e_lanes"]:
            command = tuple(lane["command"])
            if command in seen:
                continue
            seen.add(command)
            print(f"Running E2E lane for {surface_name}: {' '.join(command)}")
            try:
                subprocess.run(command, cwd=root, check=True)
            except subprocess.CalledProcessError as error:
                raise RegistryError(
                    f"E2E lane failed with exit code {error.returncode}: {' '.join(command)}"
                ) from error


def build_report(
    global_capabilities: dict[str, dict[str, Any]],
    surfaces: dict[str, dict[str, Any]],
    scenarios: dict[str, dict[str, str]],
    evidence: list[dict[str, Any]],
    overrides: list[dict[str, Any]],
) -> dict[str, Any]:
    evidence_by_contract: dict[tuple[str, str], list[dict[str, Any]]] = defaultdict(list)
    overrides_by_contract: dict[tuple[str, str], list[dict[str, Any]]] = defaultdict(list)
    for item in evidence:
        evidence_by_contract[(item["capability"], item["scenario"])].append(item)
    for item in overrides:
        overrides_by_contract[(item["capability"], item.get("scenario", "basic"))].append(item)

    capabilities = []
    for capability_id, capability in global_capabilities.items():
        scenario_rows = []
        for scenario_id, scenario_title in scenarios[capability_id].items():
            scenario_rows.append(
                {
                    "id": scenario_id,
                    "title": scenario_title,
                    "evidence": evidence_by_contract[(capability_id, scenario_id)],
                    "overrides": overrides_by_contract[(capability_id, scenario_id)],
                }
            )
        capabilities.append(
            {
                "id": capability_id,
                "title": capability.get("title", capability_id),
                "area": capability.get("area", "unknown"),
                "scenarios": scenario_rows,
            }
        )
    return {
        "version": 1,
        "surfaces": [
            {"id": name, "title": config["title"], "runtimes": list(config["runtimes"])}
            for name, config in surfaces.items()
        ],
        "capabilities": capabilities,
    }


def _surface_summary(capability: dict[str, Any], surface: str) -> str:
    kinds: set[str] = set()
    runtimes: set[str] = set()
    e2e_scenarios = 0
    for scenario in capability["scenarios"]:
        has_e2e = False
        for item in scenario["evidence"]:
            if item["surface"] != surface:
                continue
            kinds.add(item["kind"])
            runtimes.add(item["runtime"])
            has_e2e = has_e2e or item["kind"] == "e2e"
        e2e_scenarios += int(has_e2e)

    parts = []
    if "ergonomic" in kinds:
        parts.append("ergonomic")
    if "conformance" in kinds:
        parts.append("conformance")
    if e2e_scenarios:
        total = len(capability["scenarios"])
        suffix = f" {e2e_scenarios}/{total}" if total > 1 else ""
        parts.append(f"E2E{suffix}")
    if not parts:
        return "—"
    runtime_text = ", ".join(sorted(runtimes))
    return " · ".join(parts) + (f" ({runtime_text})" if runtime_text else "")


def render_markdown(report: dict[str, Any]) -> str:
    surfaces = report["surfaces"]
    lines = [
        "# Client capability evidence",
        "",
        "This file is generated from the canonical capability ledger,",
        "reference/client-contracts/catalog.json, and SKC annotations colocated",
        "with executable tests/examples. Do not edit this matrix by hand.",
        "",
        "Positive client support is derived from evidence. Human-maintained metadata is",
        "limited to blocked/not-applicable overrides in the catalog.",
        "",
        "Legend: ergonomic = handwritten facade coverage; conformance = shared semantic",
        "fixture coverage; E2E = real product/backend execution. For multi-scenario",
        "capabilities, E2E 1/3 means one of three scenarios has E2E evidence.",
        "",
    ]
    headers = ["Capability", "Area"] + [surface["title"] for surface in surfaces]
    lines.append("| " + " | ".join(headers) + " |")
    lines.append("| " + " | ".join(["---", "---"] + ["---"] * len(surfaces)) + " |")
    for capability in report["capabilities"]:
        cells = [f"{capability['title']} ({capability['id']})", capability["area"]]
        cells.extend(_surface_summary(capability, surface["id"]) for surface in surfaces)
        lines.append("| " + " | ".join(cells) + " |")

    lines.extend(["", "## Scenario evidence", ""])
    for capability in report["capabilities"]:
        if not any(item["evidence"] or item["overrides"] for item in capability["scenarios"]):
            continue
        lines.extend([f"### {capability['title']} ({capability['id']})", ""])
        for scenario in capability["scenarios"]:
            if not scenario["evidence"] and not scenario["overrides"]:
                continue
            lines.append(f"- **{scenario['title']}** ({scenario['id']})")
            for item in scenario["evidence"]:
                lines.append(
                    f"  - {item['surface']} / {item['runtime']} / {item['kind']} — "
                    f"[{item['path']}](../{item['path']}#L{item['line']})"
                )
            for item in scenario["overrides"]:
                issue = f" (#{item['issue']})" if item.get("issue") else ""
                lines.append(
                    f"  - {item['surface']} / {item['runtime']} — "
                    f"**{item['status']}**{issue}: {item['reason']}"
                )
        lines.append("")
    return "\n".join(lines) + "\n"


def generated_documents(report: dict[str, Any]) -> dict[Path, str]:
    return {
        REPORT_JSON: json.dumps(report, indent=2, sort_keys=False) + "\n",
        REPORT_MD: render_markdown(report),
    }


def check_documents(documents: dict[Path, str]) -> list[str]:
    stale = []
    for path, expected in documents.items():
        actual = path.read_text(encoding="utf-8") if path.is_file() else None
        if actual != expected:
            try:
                stale.append(path.relative_to(ROOT).as_posix())
            except ValueError:
                stale.append(str(path))
    return stale


def build_current_report(root: Path = ROOT) -> dict[str, Any]:
    global_capabilities = load_global_capabilities(root / "docs" / "capabilities.json")
    catalog = load_catalog(root / "reference" / "client-contracts" / "catalog.json")
    surfaces, scenarios, overrides = validate_catalog(catalog, global_capabilities, root)
    evidence = collect_evidence(surfaces, scenarios, root)
    validate_facade_support(global_capabilities, surfaces, evidence)
    return build_report(global_capabilities, surfaces, scenarios, evidence, overrides)


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    mode = parser.add_mutually_exclusive_group()
    mode.add_argument("--check", action="store_true")
    mode.add_argument("--write", action="store_true")
    mode.add_argument("--run-e2e", action="store_true")
    mode.add_argument("--run-e2e-surface")
    args = parser.parse_args(argv)
    try:
        if args.run_e2e or args.run_e2e_surface:
            global_capabilities = load_global_capabilities()
            catalog = load_catalog()
            surfaces, scenarios, _overrides = validate_catalog(
                catalog, global_capabilities
            )
            evidence = collect_evidence(surfaces, scenarios)
            validate_facade_support(global_capabilities, surfaces, evidence)
            run_e2e_lanes(surfaces, surface=args.run_e2e_surface)
            print("Client capability E2E lanes PASS.")
            return 0
        report = build_current_report()
        documents = generated_documents(report)
        if args.write:
            for path, content in documents.items():
                path.write_text(content, encoding="utf-8")
            print("Client capability reports updated.")
            return 0
        stale = check_documents(documents)
        if stale:
            print("Client capability reports are stale:", file=sys.stderr)
            for path in stale:
                print(f"- {path}", file=sys.stderr)
            print("Run: python3 tools/check-client-contracts.py --write", file=sys.stderr)
            return 1
        print("Client capability evidence PASS.")
        return 0
    except (RegistryError, OSError) as error:
        print(f"Client capability check FAILED: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
