#!/usr/bin/env python3
"""Verify stable checklist IDs and the derived modular audit manifest."""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import sys
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[1]
AUDIT_DIR = ROOT / "implementation-plan" / "audit"
MANIFEST_PATH = AUDIT_DIR / "split-manifest.json"
ITEM_RE = re.compile(r"^\s*-\s*\[[ xX]\]\s+\*\*(SK-[A-Z0-9]+-\d+)\*\*", re.MULTILINE)
RANGE_RE = re.compile(r"^(SK-[A-Z0-9]+-)(\d+)\.\.(SK-[A-Z0-9]+-)(\d+)$")


class AuditSplitError(ValueError):
    pass


def inventory(audit_dir: Path) -> list[dict[str, Any]]:
    files: list[dict[str, Any]] = []
    for path in sorted(audit_dir.glob("*.md")):
        content = path.read_bytes()
        text = content.decode("utf-8")
        files.append(
            {
                "file": path.name,
                "sha256": hashlib.sha256(content).hexdigest(),
                "item_count": len(ITEM_RE.findall(text)),
            }
        )
    return files


def _ids_digest(ids: list[str] | set[str]) -> str:
    payload = "".join(f"{item}\n" for item in sorted(set(ids)))
    return hashlib.sha256(payload.encode("utf-8")).hexdigest()


def _extension_ids(extension: dict[str, Any]) -> list[str]:
    ranges = extension.get("item_ranges")
    if ranges is None:
        ranges = [extension.get("added_item_range")]
    if not isinstance(ranges, list) or not ranges or not all(isinstance(item, str) for item in ranges):
        raise AuditSplitError("each live extension must declare item_ranges or added_item_range")
    result: list[str] = []
    for item_range in ranges:
        match = RANGE_RE.fullmatch(item_range)
        if not match or match.group(1) != match.group(3):
            raise AuditSplitError(f"invalid checklist ID range: {item_range}")
        prefix, start, _, end = match.groups()
        first, last = int(start), int(end)
        if last < first:
            raise AuditSplitError(f"descending checklist ID range: {item_range}")
        result.extend(f"{prefix}{number:0{len(start)}d}" for number in range(first, last + 1))
    return result


def validate_manifest(audit_dir: Path, manifest: dict[str, Any]) -> list[str]:
    errors: list[str] = []
    actual_files = inventory(audit_dir)
    expected_files = manifest.get("files")
    if expected_files != actual_files:
        expected_by_name = {item.get("file"): item for item in expected_files or [] if isinstance(item, dict)}
        actual_by_name = {item["file"]: item for item in actual_files}
        for name in sorted(set(expected_by_name) | set(actual_by_name)):
            if expected_by_name.get(name) != actual_by_name.get(name):
                errors.append(f"manifest inventory stale for {name}")

    all_ids: list[str] = []
    for item in actual_files:
        content = (audit_dir / item["file"]).read_text(encoding="utf-8")
        all_ids.extend(ITEM_RE.findall(content))
    duplicates = sorted({item for item in all_ids if all_ids.count(item) > 1})
    if duplicates:
        errors.append("duplicate checklist IDs: " + ", ".join(duplicates))
    if manifest.get("stable_item_count") != len(all_ids):
        errors.append(f"stable_item_count is {manifest.get('stable_item_count')}, expected {len(all_ids)}")
    if manifest.get("stable_item_unique_count") != len(set(all_ids)):
        errors.append(
            f"stable_item_unique_count is {manifest.get('stable_item_unique_count')}, expected {len(set(all_ids))}"
        )

    extensions = manifest.get("live_extensions", [])
    if not isinstance(extensions, list):
        errors.append("live_extensions must be a list")
        return errors
    declared_extension_ids: set[str] = set()
    for extension in extensions:
        if not isinstance(extension, dict):
            errors.append("each live extension must be an object")
            continue
        try:
            extension_ids = _extension_ids(extension)
        except AuditSplitError as error:
            errors.append(str(error))
            continue
        overlaps = sorted(declared_extension_ids & set(extension_ids))
        if overlaps:
            errors.append("checklist IDs declared by multiple extensions: " + ", ".join(overlaps))
        declared_extension_ids.update(extension_ids)
        if extension.get("added_item_count") != len(extension_ids):
            errors.append(
                f"extension {extension.get('trigger', '<unknown>')} declares "
                f"{extension.get('added_item_count')} items, ranges contain {len(extension_ids)}"
            )
        missing = sorted(set(extension_ids) - set(all_ids))
        if missing:
            errors.append("extension IDs missing from checklist: " + ", ".join(missing))

    source_item_count = manifest.get("source_item_count")
    source_item_ids_sha256 = manifest.get("source_item_ids_sha256")
    if not isinstance(source_item_count, int) or source_item_count < 0:
        errors.append("source_item_count must be a non-negative integer")
    if not isinstance(source_item_ids_sha256, str) or not source_item_ids_sha256:
        errors.append("source_item_ids_sha256 must be a non-empty string")
    if isinstance(source_item_count, int) and isinstance(source_item_ids_sha256, str):
        baseline_ids = set(all_ids) - declared_extension_ids
        if len(baseline_ids) != source_item_count:
            errors.append(
                f"baseline checklist contains {len(baseline_ids)} IDs, expected {source_item_count}"
            )
        if _ids_digest(baseline_ids) != source_item_ids_sha256:
            errors.append("baseline checklist ID set differs from the frozen source split")
    return errors


def update_manifest(audit_dir: Path, manifest: dict[str, Any]) -> dict[str, Any]:
    files = inventory(audit_dir)
    ids: list[str] = []
    for item in files:
        ids.extend(ITEM_RE.findall((audit_dir / item["file"]).read_text(encoding="utf-8")))
    manifest["files"] = files
    manifest["stable_item_count"] = len(ids)
    manifest["stable_item_unique_count"] = len(set(ids))
    return manifest


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--write", action="store_true", help="refresh derived file hashes and item counts")
    args = parser.parse_args()
    try:
        manifest = json.loads(MANIFEST_PATH.read_text(encoding="utf-8"))
        if args.write:
            manifest = update_manifest(AUDIT_DIR, manifest)
            MANIFEST_PATH.write_text(json.dumps(manifest, indent=2) + "\n", encoding="utf-8")
        errors = validate_manifest(AUDIT_DIR, manifest)
    except (OSError, json.JSONDecodeError, AuditSplitError) as error:
        print(f"audit split check failed: {error}", file=sys.stderr)
        return 1
    if errors:
        print("audit split check failed:", file=sys.stderr)
        for error in errors:
            print(f"- {error}", file=sys.stderr)
        return 1
    print(f"audit split check passed: {manifest['stable_item_unique_count']} unique IDs across {len(manifest['files'])} files")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
