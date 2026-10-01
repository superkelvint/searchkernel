#!/usr/bin/env python3
"""Regenerate and verify first-party client files mapped to audit modules."""

from __future__ import annotations

import json
import subprocess
import sys
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[1]
CATALOG_PATH = ROOT / "reference" / "client-contracts" / "catalog.json"
MANIFEST_PATH = ROOT / "implementation-plan" / "audit" / "split-manifest.json"
README_PATH = ROOT / "implementation-plan" / "audit" / "README.md"
NON_CLIENT_SURFACES = {"core", "rust"}


class ClientCoverageError(ValueError):
    pass


def _under_root(path: str, root: str) -> bool:
    return path == root or path.startswith(root.rstrip("/") + "/")


def derive_inventory(
    catalog: dict[str, Any],
    manifest: dict[str, Any],
    tracked_files: list[str],
    readme: str,
) -> tuple[list[tuple[str, str, str]], list[str]]:
    errors: list[str] = []
    surfaces = catalog.get("surfaces")
    mappings = manifest.get("client_audit_modules")
    if not isinstance(surfaces, dict) or not isinstance(mappings, dict):
        raise ClientCoverageError("catalog surfaces and client_audit_modules must be objects")
    client_ids = set(surfaces) - NON_CLIENT_SURFACES
    if client_ids != set(mappings):
        missing = sorted(client_ids - set(mappings))
        stale = sorted(set(mappings) - client_ids)
        if missing:
            errors.append("first-party clients lack audit-module mappings: " + ", ".join(missing))
        if stale:
            errors.append("audit mappings name non-current client surfaces: " + ", ".join(stale))

    manifest_files = {item.get("file") for item in manifest.get("files", []) if isinstance(item, dict)}
    inventory: list[tuple[str, str, str]] = []
    for client in sorted(client_ids & set(mappings)):
        mapping = mappings[client]
        module = mapping.get("module") if isinstance(mapping, dict) else None
        if not isinstance(module, str) or module not in manifest_files:
            errors.append(f"{client} audit module is absent from split manifest: {module!r}")
            continue
        if f"]({module})" not in readme:
            errors.append(f"{client} audit module is not indexed in audit README: {module}")
        production_roots = mapping.get("production_roots")
        support_roots = mapping.get("support_roots")
        declared_root_sets = (("production_roots", production_roots), ("support_roots", support_roots))
        invalid_roots = False
        for label, declared in declared_root_sets:
            if (
                not isinstance(declared, list)
                or not declared
                or not all(isinstance(root, str) and root.startswith(f"clients/{client}/") for root in declared)
            ):
                errors.append(f"{client} must declare non-empty {label} under clients/{client}/")
                invalid_roots = True
        if invalid_roots:
            continue
        roots = production_roots + support_roots
        client_files = [path for path in tracked_files if _under_root(path, f"clients/{client}")]
        if not client_files:
            errors.append(f"registered first-party client has no tracked package files: {client}")
            continue
        for root in roots:
            if not any(_under_root(path, root) for path in client_files):
                errors.append(f"{client} audit inventory root has no tracked files: {root}")
        for path in client_files:
            matches = [root for root in roots if _under_root(path, root)]
            if len(matches) != 1:
                errors.append(f"{path} maps to {len(matches)} audit inventory roots for {client}")
                continue
            inventory.append((client, path, module))

        package_root = Path(f"clients/{client}")
        for evidence_root in surfaces[client].get("evidence_roots", []):
            if not isinstance(evidence_root, str) or not evidence_root.startswith(f"clients/{client}/"):
                errors.append(f"{client} catalog has an out-of-package evidence root: {evidence_root!r}")
            elif not any(_under_root(evidence_root, root) for root in mapping.get("support_roots", [])):
                errors.append(f"{client} evidence root lacks audit support mapping: {evidence_root}")
        for lane in surfaces[client].get("e2e_lanes", []):
            lane_path = lane.get("path") if isinstance(lane, dict) else None
            if not isinstance(lane_path, str) or not _under_root(lane_path, f"clients/{client}"):
                errors.append(f"{client} E2E lane is outside its package root: {lane_path!r}")
            elif not any(_under_root(lane_path, root) for root in mapping.get("support_roots", [])):
                errors.append(f"{client} E2E lane lacks audit support mapping: {lane_path}")
    return inventory, errors


def main() -> int:
    try:
        catalog = json.loads(CATALOG_PATH.read_text(encoding="utf-8"))
        manifest = json.loads(MANIFEST_PATH.read_text(encoding="utf-8"))
        tracked = subprocess.run(
            ["git", "-C", str(ROOT), "ls-files", "--", "clients"],
            check=True,
            capture_output=True,
            text=True,
        ).stdout.splitlines()
        readme = README_PATH.read_text(encoding="utf-8")
        inventory, errors = derive_inventory(catalog, manifest, tracked, readme)
    except (OSError, json.JSONDecodeError, subprocess.CalledProcessError, ClientCoverageError) as error:
        print(f"client audit coverage check failed: {error}", file=sys.stderr)
        return 1
    if errors:
        print("client audit coverage check failed:", file=sys.stderr)
        for error in errors:
            print(f"- {error}", file=sys.stderr)
        return 1
    for client, path, module in inventory:
        print(f"{client}\t{path}\timplementation-plan/audit/{module}")
    print(f"client audit coverage passed: {len(inventory)} tracked files mapped across {len(set(row[0] for row in inventory))} first-party clients")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
