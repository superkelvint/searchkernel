#!/usr/bin/env python3
"""Validate executable client capability evidence and generate compatibility reports."""

from __future__ import annotations

import argparse
import importlib
import json
import re
import subprocess
import sys
import tempfile
from collections import defaultdict
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[1]
CATALOG_PATH = ROOT / "reference" / "client-contracts" / "catalog.json"
GLOBAL_LEDGER_PATH = ROOT / "docs" / "capabilities.json"
REPORT_JSON = ROOT / "docs" / "client-capabilities.json"
REPORT_MD = ROOT / "docs" / "CLIENT_CAPABILITIES.md"
VALID_KINDS = {"ergonomic", "conformance", "e2e"}
VALID_STRENGTHS = {"builder", "conformance", "runtime"}
STRENGTH_RANK = {"builder": 0, "conformance": 1, "runtime": 2}
SOURCE_SUFFIXES = {
    ".rs", ".py", ".php", ".rb", ".ts", ".tsx", ".js", ".mjs", ".c", ".cc", ".cpp", ".h", ".sh"
}
LIFECYCLE_EVIDENCE_KINDS = {"public", "wire"}
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


_REFLECTION_TEMP: tempfile.TemporaryDirectory[str] | None = None
_REFLECTION_MODULES: dict[str, Any] | None = None


def _reflection_modules() -> dict[str, Any]:
    """Generate official FlatBuffers reflection accessors with the pinned flatc."""
    global _REFLECTION_TEMP, _REFLECTION_MODULES
    if _REFLECTION_MODULES is None:
        flatc = subprocess.check_output(
            ["bash", str(ROOT / "tools" / "resolve-flatbuffers.sh"), "flatc"],
            text=True,
        ).strip()
        _REFLECTION_TEMP = tempfile.TemporaryDirectory(
            prefix="searchkernel-client-contract-bfbs-"
        )
        subprocess.run(
            [
                flatc,
                "--python",
                "-o",
                _REFLECTION_TEMP.name,
                str(ROOT / "tools" / "flatbuffers" / "reflection.fbs"),
            ],
            cwd=ROOT,
            check=True,
        )
        sys.path.insert(0, _REFLECTION_TEMP.name)
        _REFLECTION_MODULES = {
            "Schema": importlib.import_module("reflection.Schema"),
        }
    return _REFLECTION_MODULES


def _reflected_union_payloads(path: Path, union_name: str) -> tuple[str, ...]:
    """Return fully-qualified object payload names from one reflected BFBS union."""
    try:
        data = path.read_bytes()
    except OSError as error:
        raise RegistryError(f"cannot read canonical BFBS schema {path}: {error}") from error

    schema_type = _reflection_modules()["Schema"].Schema
    if not schema_type.SchemaBufferHasIdentifier(data, 0):
        raise RegistryError(f"canonical BFBS schema has invalid identifier: {path}")
    schema = schema_type.GetRootAs(data, 0)
    objects = [schema.Objects(i) for i in range(schema.ObjectsLength())]
    for i in range(schema.EnumsLength()):
        enum = schema.Enums(i)
        raw_name = enum.Name()
        name = raw_name.decode("utf-8") if raw_name else ""
        if name != union_name:
            continue
        if not enum.IsUnion():
            raise RegistryError(f"reflected declaration is not a union: {union_name}")
        payloads: list[str] = []
        for value_index in range(enum.ValuesLength()):
            value = enum.Values(value_index)
            raw_value_name = value.Name()
            value_name = raw_value_name.decode("utf-8") if raw_value_name else ""
            if value_name == "NONE":
                continue
            union_type = value.UnionType()
            if union_type is None:
                raise RegistryError(
                    f"reflected union arm has no payload type: {union_name}.{value_name}"
                )
            object_index = union_type.Index()
            if object_index < 0 or object_index >= len(objects):
                raise RegistryError(
                    f"reflected union arm payload is out of range: {union_name}.{value_name}"
                )
            reflected_object = objects[object_index]
            raw_object_name = reflected_object.Name()
            object_name = raw_object_name.decode("utf-8") if raw_object_name else ""
            if not object_name:
                raise RegistryError(
                    f"reflected union arm has unnamed payload: {union_name}.{value_name}"
                )
            payloads.append(object_name)
        return tuple(payloads)
    raise RegistryError(f"canonical BFBS schema has no union {union_name}: {path}")


def canonical_lifecycle_operations(root: Path = ROOT) -> tuple[str, ...]:
    """Read index lifecycle operations from the canonical BFBS reflection metadata."""
    bfbs_root = root / "core" / "crates" / "searchkernel-protocol" / "bfbs"
    request_bfbs = bfbs_root / "searchkernel_request.bfbs"
    response_bfbs = bfbs_root / "searchkernel_response.bfbs"
    missing = [
        path.relative_to(root).as_posix()
        for path in (request_bfbs, response_bfbs)
        if not path.is_file()
    ]
    if missing:
        raise RegistryError(
            "canonical BFBS lifecycle metadata is missing: " + ", ".join(missing)
        )

    def lifecycle_names(payloads: tuple[str, ...], suffix: str) -> tuple[str, ...]:
        result: list[str] = []
        for payload in payloads:
            name = payload.rsplit(".", 1)[-1]
            if name.endswith(suffix) and name[:-len(suffix)].endswith("Index"):
                result.append(name[:-len(suffix)])
        return tuple(result)

    request_operations = lifecycle_names(
        _reflected_union_payloads(
            request_bfbs, "SearchKernel.Protocol.RequestPayload"
        ),
        "Request",
    )
    response_operations = lifecycle_names(
        _reflected_union_payloads(
            response_bfbs, "SearchKernel.Protocol.ResponsePayload"
        ),
        "Result",
    )
    request_set = set(request_operations)
    response_set = set(response_operations)
    if request_set != response_set:
        details = []
        missing_results = sorted(request_set - response_set)
        missing_requests = sorted(response_set - request_set)
        if missing_results:
            details.append("missing result(s): " + ", ".join(missing_results))
        if missing_requests:
            details.append("missing request(s): " + ", ".join(missing_requests))
        raise RegistryError(
            "canonical BFBS lifecycle request/result unions disagree: "
            + "; ".join(details)
        )
    if not request_operations:
        raise RegistryError("canonical BFBS lifecycle unions contain no index operations")
    return request_operations


def validate_client_lifecycle_parity(
    catalog: dict[str, Any],
    root: Path = ROOT,
) -> None:
    """Require public API and wire evidence for every canonical lifecycle operation."""
    operations = canonical_lifecycle_operations(root)
    raw_surfaces = catalog.get("surfaces")
    if not isinstance(raw_surfaces, dict):
        raise RegistryError("client contract catalog must define surfaces")

    lifecycle_surfaces = catalog.get("lifecycle_surfaces")
    if (
        not isinstance(lifecycle_surfaces, list)
        or not lifecycle_surfaces
        or not all(isinstance(item, str) and item for item in lifecycle_surfaces)
    ):
        raise RegistryError(
            "client contract catalog lifecycle_surfaces must be a non-empty string list"
        )
    if len(set(lifecycle_surfaces)) != len(lifecycle_surfaces):
        raise RegistryError("client contract catalog lifecycle_surfaces contains duplicates")

    canonical_set = set(operations)
    source_cache: dict[Path, str] = {}
    for surface_name in lifecycle_surfaces:
        surface = raw_surfaces.get(surface_name)
        if not isinstance(surface, dict):
            raise RegistryError(
                f"lifecycle surface {surface_name} is not a configured client surface"
            )
        contract = surface.get("lifecycle_contract")
        if not isinstance(contract, dict) or not contract:
            raise RegistryError(f"surface {surface_name} must define lifecycle_contract")

        contract_set = set(contract)
        if contract_set != canonical_set:
            details = []
            missing_operations = sorted(canonical_set - contract_set)
            unknown_operations = sorted(contract_set - canonical_set)
            if missing_operations:
                details.append("missing operation(s): " + ", ".join(missing_operations))
            if unknown_operations:
                details.append("unknown operation(s): " + ", ".join(unknown_operations))
            raise RegistryError(
                f"surface {surface_name} lifecycle contract does not match canonical operations: "
                + "; ".join(details)
            )

        for operation in operations:
            evidence = contract.get(operation)
            if not isinstance(evidence, list) or not evidence:
                raise RegistryError(
                    f"surface {surface_name} lifecycle {operation} must define evidence"
                )
            kinds: set[str] = set()
            for evidence_index, item in enumerate(evidence):
                label = (
                    f"surface {surface_name} lifecycle {operation} "
                    f"evidence[{evidence_index}]"
                )
                if not isinstance(item, dict):
                    raise RegistryError(f"{label} must be a table")
                kind = item.get("kind")
                relative = item.get("path")
                needle = item.get("contains")
                if kind not in LIFECYCLE_EVIDENCE_KINDS:
                    raise RegistryError(
                        f"{label}.kind must be one of: "
                        + ", ".join(sorted(LIFECYCLE_EVIDENCE_KINDS))
                    )
                if not isinstance(relative, str) or not relative:
                    raise RegistryError(f"{label}.path must be a non-empty string")
                path = Path(relative)
                if path.is_absolute() or ".." in path.parts:
                    raise RegistryError(f"{label}.path must be repository-relative")
                if not isinstance(needle, str) or not needle:
                    raise RegistryError(f"{label}.contains must be a non-empty string")

                absolute = root / path
                if absolute not in source_cache:
                    try:
                        source_cache[absolute] = absolute.read_text(encoding="utf-8")
                    except OSError as error:
                        raise RegistryError(
                            f"cannot read {label}.path {relative}: {error}"
                        ) from error
                if needle not in source_cache[absolute]:
                    raise RegistryError(
                        f"{label} is missing required source evidence "
                        f"{needle!r} in {relative}"
                    )
                kinds.add(kind)

            missing_kinds = LIFECYCLE_EVIDENCE_KINDS - kinds
            if missing_kinds:
                raise RegistryError(
                    f"surface {surface_name} lifecycle {operation} "
                    "is missing evidence kind(s): "
                    + ", ".join(sorted(missing_kinds))
                )


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
                    runtime_lane = next(
                        (
                            lane
                            for lane in config["e2e_lanes"]
                            if lane["path"] == relative and runtime in lane["runtimes"]
                        ),
                        None,
                    )
                    if "e2e" in kinds and runtime_lane is None:
                        raise RegistryError(
                            f"E2E evidence at {relative}:{line_number} is not covered by an E2E CI lane"
                        )
                    for kind in kinds:
                        strength = (
                            "runtime"
                            if kind == "e2e"
                            else "conformance"
                            if kind == "conformance"
                            else "builder"
                        )
                        evidence.append(
                            {
                                "capability": capability,
                                "scenario": scenario,
                                "surface": surface,
                                "runtime": runtime,
                                "kind": kind,
                                "strength": strength,
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


def _validated_parity_requirements(
    catalog: dict[str, Any],
    surfaces: dict[str, dict[str, Any]],
) -> list[dict[str, Any]]:
    raw_requirements = catalog.get("parity_requirements", [])
    if not isinstance(raw_requirements, list):
        raise RegistryError("parity_requirements must be an array of tables")

    requirements: list[dict[str, Any]] = []
    for index, item in enumerate(raw_requirements):
        label = f"parity_requirements[{index}]"
        if not isinstance(item, dict):
            raise RegistryError(f"{label} must be a table")
        source = item.get("source_surface")
        target = item.get("target_surface")
        strength = item.get("minimum_strength")
        if source not in surfaces:
            raise RegistryError(f"{label}.source_surface is unknown: {source}")
        if target not in surfaces:
            raise RegistryError(f"{label}.target_surface is unknown: {target}")
        if source == target:
            raise RegistryError(f"{label} must compare different surfaces")
        if strength not in VALID_STRENGTHS:
            raise RegistryError(
                f"{label}.minimum_strength must be one of {sorted(VALID_STRENGTHS)}"
            )

        normalized = dict(item)
        required_kinds = item.get("required_kinds", [])
        if not isinstance(required_kinds, list) or not all(
            isinstance(kind, str) and kind in VALID_KINDS for kind in required_kinds
        ):
            raise RegistryError(
                f"{label}.required_kinds must be a list drawn from {sorted(VALID_KINDS)}"
            )
        if len(set(required_kinds)) != len(required_kinds):
            raise RegistryError(f"{label}.required_kinds contains duplicates")
        normalized["required_kinds"] = list(required_kinds)
        required_contracts = item.get("required_contracts", [])
        contract_re = re.compile(
            r"[a-z][a-z0-9_.-]*\.[a-z0-9_.-]+/[a-z0-9][a-z0-9_.-]*"
        )
        if not isinstance(required_contracts, list) or not all(
            isinstance(contract, str) and contract_re.fullmatch(contract)
            for contract in required_contracts
        ):
            raise RegistryError(
                f"{label}.required_contracts must be capability/scenario strings"
            )
        if len(set(required_contracts)) != len(required_contracts):
            raise RegistryError(f"{label}.required_contracts contains duplicates")
        normalized["required_contracts"] = list(required_contracts)
        for field, surface in (
            ("source_runtimes", source),
            ("target_runtimes", target),
        ):
            runtimes = item.get(field)
            if not isinstance(runtimes, list) or not runtimes or not all(
                isinstance(runtime, str) and runtime in surfaces[surface]["runtimes"]
                for runtime in runtimes
            ):
                raise RegistryError(
                    f"{label}.{field} must be a non-empty subset of {surface} runtimes"
                )
            if len(set(runtimes)) != len(runtimes):
                raise RegistryError(f"{label}.{field} contains duplicates")
            normalized[field] = list(runtimes)
        requirements.append(normalized)
    return requirements


def validate_parity_evidence(
    catalog: dict[str, Any],
    surfaces: dict[str, dict[str, Any]],
    evidence: list[dict[str, Any]],
    overrides: list[dict[str, Any]],
) -> None:
    """Fail closed when a target surface has weaker evidence than its parity source."""
    requirements = _validated_parity_requirements(catalog, surfaces)
    evidence_by_contract: dict[tuple[str, str], list[dict[str, Any]]] = defaultdict(list)
    for item in evidence:
        evidence_by_contract[(item["capability"], item["scenario"])].append(item)

    def profiles(
        contract_evidence: list[dict[str, Any]],
        surface: str,
        runtimes: set[str],
    ) -> list[dict[str, Any]]:
        grouped: dict[tuple[str, str, int], dict[str, Any]] = {}
        for item in contract_evidence:
            if item["surface"] != surface or item["runtime"] not in runtimes:
                continue
            key = (item["runtime"], item["path"], item["line"])
            profile = grouped.setdefault(
                key,
                {
                    "runtime": item["runtime"],
                    "path": item["path"],
                    "line": item["line"],
                    "kinds": set(),
                    "max_strength": "builder",
                },
            )
            profile["kinds"].add(item["kind"])
            if STRENGTH_RANK[item["strength"]] > STRENGTH_RANK[profile["max_strength"]]:
                profile["max_strength"] = item["strength"]
        return list(grouped.values())

    for requirement in requirements:
        source = requirement["source_surface"]
        target = requirement["target_surface"]
        minimum = requirement["minimum_strength"]
        required_kinds = set(requirement["required_kinds"])
        required_contracts = [
            tuple(contract.rsplit("/", 1))
            for contract in requirement["required_contracts"]
        ]
        source_runtimes = set(requirement["source_runtimes"])
        target_runtimes = set(requirement["target_runtimes"])
        minimum_rank = STRENGTH_RANK[minimum]

        for capability, scenario in required_contracts:
            contract_evidence = evidence_by_contract.get((capability, scenario), [])
            source_profiles = [
                profile
                for profile in profiles(contract_evidence, source, source_runtimes)
                if STRENGTH_RANK[profile["max_strength"]] >= minimum_rank
                and required_kinds.issubset(profile["kinds"])
            ]
            if not source_profiles:
                raise RegistryError(
                    f"required source parity evidence missing for {capability}/{scenario}: "
                    f"{source} needs {minimum} evidence with colocated kinds "
                    f"{sorted(required_kinds)} on an allowed source runtime"
                )

        for (capability, scenario), contract_evidence in sorted(evidence_by_contract.items()):
            source_profiles = [
                profile
                for profile in profiles(contract_evidence, source, source_runtimes)
                if STRENGTH_RANK[profile["max_strength"]] >= minimum_rank
                and required_kinds.issubset(profile["kinds"])
            ]
            if not source_profiles:
                continue

            target_profiles = [
                profile
                for profile in profiles(contract_evidence, target, target_runtimes)
                if STRENGTH_RANK[profile["max_strength"]] >= minimum_rank
                and required_kinds.issubset(profile["kinds"])
            ]
            matching_overrides = [
                item
                for item in overrides
                if item["capability"] == capability
                and item.get("scenario", "basic") == scenario
                and item["surface"] == target
                and item["runtime"] in target_runtimes
            ]
            if target_profiles:
                if matching_overrides:
                    raise RegistryError(
                        f"stale parity override for {capability}/{scenario}: "
                        f"{target} now has {minimum} evidence"
                    )
                continue
            if matching_overrides:
                continue

            target_evidence = [
                item for item in contract_evidence if item["surface"] == target
            ]
            if target_evidence:
                target_groups = profiles(
                    contract_evidence,
                    target,
                    {item["runtime"] for item in target_evidence},
                )
                profile_text = ", ".join(
                    sorted(
                        f"{profile['runtime']}/{profile['max_strength']} "
                        f"({'+'.join(sorted(profile['kinds']))})"
                        for profile in target_groups
                    )
                )
                target_detail = f"only weaker or split evidence: {profile_text}"
            else:
                target_detail = "no evidence"
            source_text = ", ".join(
                sorted(
                    f"{profile['runtime']}/{profile['max_strength']} "
                    f"({'+'.join(sorted(profile['kinds']))})"
                    for profile in source_profiles
                )
            )
            required_text = (
                f" with colocated kinds {sorted(required_kinds)}"
                if required_kinds
                else ""
            )
            raise RegistryError(
                f"{minimum} parity mismatch for {capability}/{scenario}: "
                f"{source} has {source_text}; {target} has {target_detail}. "
                f"Add matching {minimum} evidence{required_text} on an allowed "
                "target runtime or an explicit override."
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
    runtime_scenarios = 0
    for scenario in capability["scenarios"]:
        has_runtime = False
        for item in scenario["evidence"]:
            if item["surface"] != surface:
                continue
            kinds.add(item["kind"])
            runtimes.add(item["runtime"])
            has_runtime = has_runtime or item["strength"] == "runtime"
        runtime_scenarios += int(has_runtime)

    parts = []
    if "ergonomic" in kinds:
        parts.append("ergonomic")
    if "conformance" in kinds:
        parts.append("conformance")
    if runtime_scenarios:
        total = len(capability["scenarios"])
        suffix = f" {runtime_scenarios}/{total}" if total > 1 else ""
        parts.append(f"real-runtime{suffix}")
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
        "fixture coverage; real-runtime = evidence owned by a registered E2E lane/runtime.",
        "For multi-scenario capabilities, real-runtime 1/3 means one of three scenarios",
        "has real-runtime evidence.",
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
                    f"  - {item['surface']} / {item['runtime']} / {item['kind']} / "
                    f"{item['strength']} — "
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
    validate_client_lifecycle_parity(catalog, root)
    surfaces, scenarios, overrides = validate_catalog(catalog, global_capabilities, root)
    evidence = collect_evidence(surfaces, scenarios, root)
    validate_facade_support(global_capabilities, surfaces, evidence)
    validate_parity_evidence(catalog, surfaces, evidence, overrides)
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
            validate_client_lifecycle_parity(catalog)
            surfaces, scenarios, _overrides = validate_catalog(
                catalog, global_capabilities
            )
            evidence = collect_evidence(surfaces, scenarios)
            validate_facade_support(global_capabilities, surfaces, evidence)
            validate_parity_evidence(catalog, surfaces, evidence, _overrides)
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
            print("Run: ./dev bookkeeping sync", file=sys.stderr)
            return 1
        print("Client capability evidence PASS.")
        return 0
    except (RegistryError, OSError) as error:
        print(f"Client capability check FAILED: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
