#!/usr/bin/env python3
"""Validate and deterministically repair structural coverage in the agent map."""

from __future__ import annotations

import argparse
import copy
import json
import pathlib
import subprocess
import re
import sys
from typing import NamedTuple

ROOT = pathlib.Path(__file__).resolve().parents[1]
REGISTRY = ROOT / "build" / "targets.json"


class Boundary(NamedTuple):
    kind: str
    path: str


def load_registry(path: pathlib.Path = REGISTRY) -> dict:
    return json.loads(path.read_text(encoding="utf-8"))


def tracked_paths(root: pathlib.Path = ROOT) -> set[str]:
    output = subprocess.check_output(
        ["git", "ls-files", "-z"], cwd=root
    )
    return {
        item.decode("utf-8")
        for item in output.split(b"\0")
        if item
    }


def workspace_members(root: pathlib.Path = ROOT) -> list[str]:
    text = (root / "Cargo.toml").read_text(encoding="utf-8")
    workspace = re.search(
        r"(?ms)^\\[workspace\\]\\s*(.*?)(?=^\\[|\\Z)", text
    )
    if workspace is None:
        raise ValueError("Cargo.toml is missing [workspace]")
    members = re.search(
        r"(?ms)^members\\s*=\\s*\\[(.*?)\\]", workspace.group(1)
    )
    if members is None:
        raise ValueError("Cargo.toml [workspace] is missing members")
    return re.findall(r'"([^"]+)"', members.group(1))


def immediate_tracked_dirs(paths: set[str], prefix: str) -> list[str]:
    result = set()
    for path in paths:
        if not path.startswith(prefix):
            continue
        remainder = path[len(prefix):]
        if "/" not in remainder:
            continue
        result.add(remainder.split("/", 1)[0])
    return sorted(result)


def structural_boundaries(
    paths: set[str],
    members: list[str],
) -> list[Boundary]:
    top_level = sorted(
        {
            path.split("/", 1)[0]
            for path in paths
            if "/" in path
        }
    )
    result = [Boundary("top-level", f"{name}/") for name in top_level]
    result.extend(
        Boundary("workspace", member.rstrip("/") + "/")
        for member in members
    )
    result.extend(
        Boundary("client", f"clients/{name}/")
        for name in immediate_tracked_dirs(paths, "clients/")
    )
    result.extend(
        Boundary("integration", f"integrations/{name}/")
        for name in immediate_tracked_dirs(paths, "integrations/")
    )

    deduped: dict[tuple[str, str], Boundary] = {}
    for boundary in result:
        deduped[(boundary.kind, boundary.path)] = boundary
    return list(deduped.values())


def missing_boundaries(data: dict, boundaries: list[Boundary]) -> list[Boundary]:
    areas = data["change_areas"]
    return [
        boundary
        for boundary in boundaries
        if boundary.path not in areas
    ]


def _clone_template(data: dict, prefix: str) -> dict:
    try:
        return copy.deepcopy(data["change_areas"][prefix])
    except KeyError as error:
        raise ValueError(
            f"agent-map autofix template is missing: {prefix}"
        ) from error


def _read_path_for(
    tracked: set[str],
    directory: str,
    fallback: str,
) -> list[str]:
    readme = directory.rstrip("/") + "/README.md"
    return [readme] if readme in tracked else [fallback]


def _autofix_boundary(
    data: dict,
    boundary: Boundary,
    tracked: set[str],
) -> tuple[dict | None, str | None]:
    path = boundary.path
    name = path.rstrip("/").split("/")[-1]

    if boundary.kind == "workspace" and path.startswith("core/crates/"):
        meta = _clone_template(data, "core/crates/")
        meta["owner"] = name
        meta["area"] = f"Rust crate: {name}"
        meta["read"] = _read_path_for(
            tracked, path, "core/crates/README.md"
        )
        meta["rules"] = [
            "Start from the Rust-workspace defaults; replace generic checks when a crate gains a dedicated verification lane."
        ]
        return meta, None

    if boundary.kind == "client":
        if name not in data["test"]:
            return None, (
                f"{path} is a new client boundary but test:{name} is not "
                "registered; add the client test target before ownership can "
                "be generated safely"
            )
        meta = _clone_template(data, "clients/")
        meta["owner"] = name
        meta["area"] = f"{name.title()} client"
        meta["read"] = _read_path_for(
            tracked, path, "clients/README.md"
        )
        meta["test"] = [f"test:{name}"]
        meta["verify"] = [
            "verify:contracts",
            "verify:client-semantic-parity",
        ]
        meta["focused_check"] = "verify:contracts"
        meta["strong_check"] = f"test:{name}"
        meta["rules"] = [
            "Keep the client a thin facade over canonical FlatBuffers and transport."
        ]
        return meta, None

    if boundary.kind == "integration":
        meta = _clone_template(data, "integrations/")
        meta["owner"] = name
        meta["area"] = f"{name.title()} integration"
        meta["read"] = _read_path_for(
            tracked, path, "docs/architecture.md"
        )
        return meta, None

    if boundary.kind == "workspace":
        return None, (
            f"{path} is a new workspace member outside core/crates/; "
            "its ownership and strong verifier require an explicit decision"
        )

    if boundary.kind == "top-level":
        return None, (
            f"{path} is a new top-level repository area; ownership and "
            "verification require an explicit map entry"
        )

    return None, f"unsupported boundary kind {boundary.kind}: {path}"


def synchronized_registry(
    data: dict,
    boundaries: list[Boundary],
    tracked: set[str],
) -> tuple[dict, list[str], list[str]]:
    result = copy.deepcopy(data)
    added: list[str] = []
    unresolved: list[str] = []

    # A child boundary can also be reported as top-level. Top-level coverage is
    # satisfied once an explicit top-level template exists; only auto-fix the
    # more specific structural boundary.
    missing = missing_boundaries(result, boundaries)
    for boundary in sorted(missing, key=lambda item: (item.path, item.kind)):
        if boundary.path in result["change_areas"]:
            continue
        meta, error = _autofix_boundary(result, boundary, tracked)
        if meta is None:
            unresolved.append(error or boundary.path)
            continue
        result["change_areas"][boundary.path] = meta
        added.append(boundary.path)

    return result, added, unresolved


def render(data: dict) -> str:
    return json.dumps(data, indent=2) + "\n"


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    mode = parser.add_mutually_exclusive_group()
    mode.add_argument("--check", action="store_true")
    mode.add_argument("--write", action="store_true")
    args = parser.parse_args(argv)

    data = load_registry()
    tracked = tracked_paths()
    boundaries = structural_boundaries(tracked, workspace_members())
    missing = missing_boundaries(data, boundaries)

    if not missing:
        print("Agent map structural coverage PASS.")
        return 0

    synchronized, added, unresolved = synchronized_registry(
        data, boundaries, tracked
    )

    if unresolved:
        print("Agent map requires explicit decisions:", file=sys.stderr)
        for message in unresolved:
            print(f"- {message}", file=sys.stderr)
        return 2

    if args.write:
        REGISTRY.write_text(render(synchronized), encoding="utf-8")
        if added:
            print("Agent map updated:")
            for path in added:
                print(f"- {path}")
        return 0

    print("Agent map has deterministic structural drift:", file=sys.stderr)
    for path in added:
        print(f"- {path}", file=sys.stderr)
    print("Run: ./dev bookkeeping sync", file=sys.stderr)
    return 1


if __name__ == "__main__":
    raise SystemExit(main())
