#!/usr/bin/env python3
"""Exact-head and change-aware behavioral verification helpers for CI."""

from __future__ import annotations

import argparse
import fnmatch
import json
import subprocess
import sys
from pathlib import Path
from typing import Iterable

LANES = (
    "rust",
    "python",
    "typescript",
    "php",
    "native",
    "protocol",
    "api_conformance",
    "contracts",
    "abi",
    "persistence",
    "issue_queue",
    "verification",
)

LANE_PATTERNS: dict[str, tuple[str, ...]] = {
    "rust": (
        "Cargo.toml",
        "Cargo.lock",
        ".cargo/**",
        "rust-toolchain*",
        "api/**",
        "core/**",
        "searchkerneld/**",
        "dev",
    ),
    "python": (
        "api/**",
        "clients/python/**",
        "reference/client-contracts/**",
        "tools/check-client-contracts.py",
        "tools/test_check_client_contracts.py",
        "tools/check-python-*.py",
        "tools/test_python_*.py",
        "tools/generate-python-*.sh",
    ),
    "typescript": (
        "api/**",
        "clients/typescript/**",
        "reference/client-contracts/**",
        "tools/check-client-contracts.py",
        "tools/test_check_client_contracts.py",
        "tools/generate-typescript-*.sh",
    ),
    "php": (
        "api/**",
        "clients/php/**",
        "reference/client-contracts/**",
        "tools/check-client-contracts.py",
        "tools/test_check_client_contracts.py",
    ),
    "native": (
        "core/native/**",
        "core/crates/searchkernel-vespa-bindings/**",
        "core/crates/searchkernel-vespa-engine/**",
        "core/crates/searchkernel-ir-adapter/**",
        "core/crates/searchkernel/Cargo.toml",
        "core/crates/searchkernel/src/index/native.rs",
        "core/crates/searchkernel/src/index/manifest.rs",
        "core/crates/searchkernel/src/portable_definition.rs",
        "searchkerneld/Cargo.toml",
        "searchkerneld/src/main.rs",
        "reference/vespa/**",
        "tools/check-vespa-bindings.sh",
        "tools/check-native-*.sh",
        "tools/check-native-*.py",
        "tools/test-native-*.sh",
        "tools/test_check_native_*.py",
        "tools/native-*.py",
        "tools/*native-sdk*.sh",
        "tools/*native-base*.sh",
        "tools/*shared-library*.sh",
        "tools/*native-abi*.sh",
        "tools/run-python-native-e2e.sh",
        "tools/coverage-native-adapter.sh",
    ),
    "protocol": (
        "api/**",
        "core/crates/searchkernel-protocol/src/generated/**",
        "core/crates/searchkernel-protocol/bfbs/**",
        "searchkerneld/src/generated/**",
        "clients/python/searchkernel/_wire_generated/**",
        "clients/python/searchkernel/rpc/**",
        "clients/typescript/src/generated/**",
        "tools/rpc-codegen/**",
        "tools/generate-searchkernel-protocol.sh",
        "tools/generate-flatbuffers-bfbs.sh",
        "tools/generate-python-flatbuffers.sh",
        "tools/generate-python-grpc.sh",
        "tools/generate-searchkernel-rpc.sh",
        "tools/generate-typescript-flatbuffers.sh",
        "tools/generate-typescript-grpc-web.sh",
        "tools/check-flatbuffers-bfbs.sh",
        "tools/check-searchkernel-protocol-generated.sh",
        "tools/check-searchkernel-rpc.sh",
        "tools/check-python-flatbuffers-generated.sh",
        "tools/check-python-grpc.sh",
        "tools/check-typescript-generated.sh",
        "tools/patch-python-flatbuffers-object-api.py",
    ),
    "api_conformance": (
        "reference/api-conformance/**",
    ),
    "contracts": (
        "clients/**",
        "core/crates/searchkernel/**",
        "core/crates/searchkernel-vespa-engine/**",
        "docs/capabilities.json",
        "docs/CAPABILITIES.md",
        "LIMITATIONS.md",
        "docs/CLIENT_CAPABILITIES.md",
        "docs/client-capabilities.json",
        "reference/client-contracts/**",
        "tools/check-capabilities.py",
        "tools/check-client-contracts.py",
        "tools/test_check_capabilities.py",
        "tools/test_check_client_contracts.py",
        "tools/check-python-dependencies.py",
        "tools/test_python_dependency_contract.py",
        ".github/ci/select-python.sh",
        ".github/ci/check-python-workflows.sh",
        ".github/workflows/client-capabilities.yml",
        "dev",
    ),
    "abi": (
        "core/native/**",
        "core/crates/searchkernel-vespa-bindings/**",
        "tools/check_abi_surface.py",
        "tools/test_check_abi_surface.py",
        "tools/check-native-bindings.sh",
        "tools/check-native-bindings/**",
        "tools/check-native-*.py",
        "tools/test_check_native_*.py",
        "tools/check-layout.sh",
        "tools/native-build-fingerprint.py",
        "tools/*native-sdk*.sh",
        "tools/*native-base*.sh",
        "tools/*shared-library*.sh",
        "tools/*native-abi*.sh",
        ".github/workflows/abi-surface.yml",
        "dev",
    ),
    "persistence": (
        "api/searchkernel.fbs",
        "core/crates/searchkernel/src/index/**",
        "core/crates/searchkernel/src/tests.rs",
        "core/crates/searchkernel/src/schema.rs",
        "core/crates/searchkernel/src/portable_definition.rs",
        "core/crates/searchkernel/src/search.rs",
        "core/crates/searchkernel/src/filter.rs",
        "core/crates/searchkernel/src/result.rs",
        "core/crates/searchkernel/tests/persistence_compatibility.rs",
        "core/crates/searchkernel-ir/src/api_to_ir/schema.rs",
        "core/crates/searchkernel-ir/src/ir_to_api/schema.rs",
        "core/crates/searchkernel-ir/src/model/schema.rs",
        "core/crates/searchkernel-ir/src/snapshot.rs",
        "core/crates/searchkernel-ir-adapter/**",
        "core/crates/searchkernel-vespa-engine/**",
        "core/crates/searchkernel-vespa-bindings/**",
        "core/native/vespa/**",
        "reference/persistence/**",
        "reference/vespa/m5/fixtures/persisted-v1/**",
        "tools/check-persistence-fixtures.sh",
        "tools/check-persistence-fixture-immutability.sh",
        "tools/check-persistence-sensitivity.sh",
        ".github/workflows/persistence-compatibility.yml",
    ),
    "issue_queue": (
        "AGENTS.md",
        "tools/check-issue-queue.py",
        "tools/test_check_issue_queue.py",
        ".github/ci/select-python.sh",
        ".github/ci/check-python-workflows.sh",
        ".github/workflows/issue-queue.yml",
        "dev",
    ),
    "verification": (
        ".githooks/pre-commit",
        ".github/ci/exact_head.py",
        ".github/ci/test_exact_head.py",
        "dev",
    ),
}

# Paths in these categories may legitimately change without selecting a
# behavioral verification lane. Any path not covered by this allowlist or by a
# known lane fails closed below.
NON_BEHAVIORAL_PATTERNS: tuple[str, ...] = (
    "README.md",
    "**/README.md",
    "**/AGENTS.md",
    "docs/*.md",
    "docs/**/*.md",
    "docs/*.txt",
    "docs/**/*.txt",
    "implementation-plan/*.md",
    "implementation-plan/**/*.md",
    "audits/*.md",
    "audits/**/*.md",
    ".gitignore",
    ".rgignore",
    "LICENSE*",
    "NOTICE*",
    "*.tmp",
    "*.log",
    "*.swp",
    ".DS_Store",
)

# Changes to the verification control plane must never be allowed to select
# their own reduced verification set. CI still executes the trusted base
# revision of this classifier; locally these paths conservatively select every
# current lane.
ELEVATED_VERIFICATION_PATTERNS: tuple[str, ...] = (
    ".githooks/**",
    ".github/ci/**",
    ".github/workflows/**",
    ".moon/**",
    "moon.yml",
    "moon.yaml",
    "dev",
)

PORTABLE_SCHEMA_PATTERNS: tuple[str, ...] = (
    "api/*.fbs",
    "api/**/*.fbs",
)

PROTOCOL_TOOLING_PATTERNS: tuple[str, ...] = (
    "api/rpc/**",
    "core/crates/searchkernel-protocol/**",
    "tools/rpc-codegen/**",
    "tools/generate-*protocol*.sh",
    "tools/generate-*flatbuffers*.sh",
    "tools/generate-*grpc*.sh",
    "tools/check-*protocol*.sh",
    "tools/check-*flatbuffers*.sh",
    "tools/check-*grpc*.sh",
)

CLIENT_CONTRACT_PATTERNS: tuple[str, ...] = (
    "reference/client-contracts/**",
    "tools/check-client-contracts.py",
    "tools/test_check_client_contracts.py",
)

GENERATED_ARTIFACT_PATTERNS: tuple[str, ...] = (
    "core/crates/searchkernel-protocol/src/generated/**",
    "core/crates/searchkernel-protocol/bfbs/**",
    "searchkerneld/src/generated/**",
    "clients/python/searchkernel/_wire_generated/**",
    "clients/python/searchkernel/rpc/**",
    "clients/typescript/src/generated/**",
)

NATIVE_BEHAVIOR_PATTERNS: tuple[str, ...] = (
    "core/native/**",
    "core/crates/searchkernel-vespa-bindings/**",
    "core/crates/searchkernel-vespa-engine/**",
    "core/crates/searchkernel-ir-adapter/**",
    "core/crates/searchkernel/src/index/native.rs",
    "core/crates/searchkernel/src/index/manifest.rs",
    "core/crates/searchkernel/src/portable_definition.rs",
    "searchkerneld/**",
    "reference/vespa/**",
    "tools/check-vespa-bindings.sh",
    "tools/check-native-*.sh",
    "tools/check-native-*.py",
    "tools/test-native-*.sh",
    "tools/test_check_native_*.py",
    "tools/native-*.py",
    "tools/*native-sdk*.sh",
    "tools/*native-base*.sh",
    "tools/*shared-library*.sh",
    "tools/*native-abi*.sh",
    "tools/run-python-native-e2e.sh",
    "tools/coverage-native-adapter.sh",
)

ABI_SENSITIVE_PATTERNS: tuple[str, ...] = (
    "core/native/**",
    "core/crates/searchkernel-vespa-bindings/**",
    "tools/check-native-*.sh",
    "tools/check-native-*.py",
    "tools/test-native-*.sh",
    "tools/test_check_native_*.py",
    "tools/native-*.py",
    "tools/*native-sdk*.sh",
    "tools/*native-base*.sh",
    "tools/*shared-library*.sh",
    "tools/*native-abi*.sh",
)

PERSISTENCE_STORAGE_PATTERNS: tuple[str, ...] = (
    "reference/persistence/**",
    "reference/vespa/m5/fixtures/persisted-v1/**",
    "core/crates/searchkernel/src/index/**",
    "core/crates/searchkernel/tests/persistence_compatibility.rs",
    "core/crates/searchkernel-ir/src/snapshot.rs",
    "tools/check-persistence-*.sh",
)

BUILD_SYSTEM_PATTERNS: tuple[str, ...] = (
    "Cargo.toml",
    "Cargo.lock",
    ".cargo/**",
    "rust-toolchain*",
)

KNOWN_CORE_PATTERNS: tuple[str, ...] = (
    "core/native/**",
    "core/crates/searchkernel-dispatch/**",
    "core/crates/searchkernel-filter/**",
    "core/crates/searchkernel-ir/**",
    "core/crates/searchkernel-ir-adapter/**",
    "core/crates/searchkernel-protocol/**",
    "core/crates/searchkernel-testkit/**",
    "core/crates/searchkernel-vespa-bindings/**",
    "core/crates/searchkernel-vespa-engine/**",
    "core/crates/searchkernel/**",
)

KNOWN_CLIENT_PATTERNS: tuple[str, ...] = (
    "clients/python/**",
    "clients/typescript/**",
    "clients/php/**",
)

# Stable task IDs are the trusted plan vocabulary. Phase 3 will bind these IDs
# to the canonical moon task graph, but candidate task configuration never
# participates in deciding which IDs are mandatory.
TASKS_BY_LANE: dict[str, tuple[str, ...]] = {
    "rust": ("rust-non-native",),
    "python": ("python-client",),
    "typescript": ("typescript-client",),
    "php": ("php-client",),
    "native": ("vespa-integration", "native-python-e2e"),
    "protocol": ("protocol-generated",),
    "api_conformance": ("api-conformance",),
    "contracts": ("client-contracts",),
    "abi": ("abi",),
    "persistence": ("persistence-native",),
    "issue_queue": ("issue-queue",),
    "verification": ("verification-policy",),
}


def _require(selected: dict[str, bool], *lanes: str) -> None:
    for lane in lanes:
        selected[lane] = True


def _require_all(selected: dict[str, bool]) -> None:
    _require(selected, *LANES)


def _git(repo: Path, *args: str) -> str:
    completed = subprocess.run(
        ["git", *args],
        cwd=repo,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        check=False,
    )
    if completed.returncode != 0:
        raise RuntimeError(completed.stdout.strip() or "git command failed")
    return completed.stdout.strip()


def assert_exact_head(expected: str, repo: Path) -> int:
    expected = expected.strip().lower()
    if not expected:
        print("::error title=Exact-head verification::Expected head SHA is empty.")
        return 2
    try:
        actual = _git(repo, "rev-parse", "HEAD").lower()
    except RuntimeError as error:
        print(f"::error title=Exact-head verification::{error}")
        return 2

    if actual != expected:
        print(
            "::error title=Stale verification checkout::"
            f"Expected exact PR head {expected}, checked out {actual}. "
            "Verification from a different SHA cannot authorize this head."
        )
        return 1

    print(f"Verified exact PR head {actual}.")
    return 0


def _matches(path: str, patterns: Iterable[str]) -> bool:
    return any(fnmatch.fnmatchcase(path, pattern) for pattern in patterns)


def _is_known_behavioral_path(path: str) -> bool:
    # Broad compatibility patterns such as core/** and clients/** are useful
    # lane selectors but must not make a newly introduced architecture area
    # look known to an older trusted-base policy.
    if path.startswith("core/"):
        return _matches(path, KNOWN_CORE_PATTERNS)
    if path.startswith("clients/"):
        return _matches(path, KNOWN_CLIENT_PATTERNS)
    return any(_matches(path, LANE_PATTERNS[lane]) for lane in LANES)


def classify(paths: Iterable[str]) -> dict[str, bool]:
    normalized = [path.strip().replace("\\", "/") for path in paths if path.strip()]
    selected = {
        lane: any(_matches(path, LANE_PATTERNS[lane]) for path in normalized)
        for lane in LANES
    }

    for path in normalized:
        # Verification-policy and orchestration changes get the strongest
        # current selection. The authoritative CI copy of this policy still
        # comes from the trusted base revision.
        if _matches(path, ELEVATED_VERIFICATION_PATTERNS):
            _require_all(selected)
            continue

        # Root Rust/build metadata can change feature resolution, native
        # linkage, code generation, or workspace membership. Treat it as a
        # whole-repository behavioral change rather than guessing which leaf
        # lane is affected.
        if _matches(path, BUILD_SYSTEM_PATTERNS):
            _require_all(selected)

        if _matches(path, PORTABLE_SCHEMA_PATTERNS):
            _require(
                selected,
                "rust",
                "python",
                "typescript",
                "php",
                "native",
                "protocol",
                "api_conformance",
                "contracts",
                "persistence",
            )

        if _matches(path, PROTOCOL_TOOLING_PATTERNS):
            _require(selected, "rust", "python", "typescript", "protocol", "contracts")

        if _matches(path, CLIENT_CONTRACT_PATTERNS):
            _require(selected, "python", "typescript", "php", "contracts")

        if _matches(path, GENERATED_ARTIFACT_PATTERNS):
            _require(selected, "rust", "python", "typescript", "protocol", "contracts")

        if _matches(path, NATIVE_BEHAVIOR_PATTERNS):
            _require(selected, "rust", "native", "persistence", "contracts")

        if _matches(path, ABI_SENSITIVE_PATTERNS):
            _require(selected, "abi")

        if _matches(path, PERSISTENCE_STORAGE_PATTERNS):
            _require(selected, "rust", "native", "persistence")

        known_behavioral_path = _is_known_behavioral_path(path)
        explicitly_non_behavioral = _matches(path, NON_BEHAVIORAL_PATTERNS)
        if not known_behavioral_path and not explicitly_non_behavioral:
            # Fail closed for new source/build/tooling paths that the trusted
            # base policy does not know yet. A future policy revision can
            # classify the path more narrowly after this candidate lands.
            _require_all(selected)

    return selected


def _selection_reasons(path: str, lane: str) -> list[str]:
    reasons: list[str] = []

    if _matches(path, LANE_PATTERNS[lane]):
        reasons.append(f"{path}: matches {lane} lane")

    if _matches(path, ELEVATED_VERIFICATION_PATTERNS):
        reasons.append(f"{path}: verification/task orchestration change")

    if _matches(path, BUILD_SYSTEM_PATTERNS):
        reasons.append(f"{path}: root build/toolchain metadata")

    portable_schema_lanes = {
        "rust",
        "python",
        "typescript",
        "php",
        "native",
        "protocol",
        "api_conformance",
        "contracts",
        "persistence",
    }
    if lane in portable_schema_lanes and _matches(path, PORTABLE_SCHEMA_PATTERNS):
        reasons.append(f"{path}: portable schema fan-out")

    protocol_lanes = {"rust", "python", "typescript", "protocol", "contracts"}
    if lane in protocol_lanes and _matches(path, PROTOCOL_TOOLING_PATTERNS):
        reasons.append(f"{path}: protocol/tooling fan-out")

    client_contract_lanes = {"python", "typescript", "php", "contracts"}
    if lane in client_contract_lanes and _matches(path, CLIENT_CONTRACT_PATTERNS):
        reasons.append(f"{path}: shared client-contract fan-out")

    generated_lanes = {"rust", "python", "typescript", "protocol", "contracts"}
    if lane in generated_lanes and _matches(path, GENERATED_ARTIFACT_PATTERNS):
        reasons.append(f"{path}: generated-artifact fan-out")

    native_lanes = {"rust", "native", "persistence", "contracts"}
    if lane in native_lanes and _matches(path, NATIVE_BEHAVIOR_PATTERNS):
        reasons.append(f"{path}: native behavior fan-out")

    if lane == "abi" and _matches(path, ABI_SENSITIVE_PATTERNS):
        reasons.append(f"{path}: ABI-sensitive fan-out")

    persistence_lanes = {"rust", "native", "persistence"}
    if lane in persistence_lanes and _matches(path, PERSISTENCE_STORAGE_PATTERNS):
        reasons.append(f"{path}: persistence/storage fan-out")

    known_behavioral_path = _is_known_behavioral_path(path)
    explicitly_non_behavioral = _matches(path, NON_BEHAVIORAL_PATTERNS)
    if not known_behavioral_path and not explicitly_non_behavioral:
        reasons.append(f"{path}: unclassified behavioral path, fail closed")

    return reasons


def build_task_plan(paths: Iterable[str]) -> list[dict[str, object]]:
    normalized = [path.strip().replace("\\", "/") for path in paths if path.strip()]
    selected = classify(normalized)
    task_reasons: dict[str, list[str]] = {}

    for lane in LANES:
        if not selected[lane]:
            continue
        lane_reasons: list[str] = []
        for path in normalized:
            lane_reasons.extend(_selection_reasons(path, lane))
        if not lane_reasons:
            lane_reasons.append(f"selected by {lane} policy")
        deduped = list(dict.fromkeys(lane_reasons))
        for task_id in TASKS_BY_LANE[lane]:
            task_reasons.setdefault(task_id, [])
            task_reasons[task_id].extend(deduped)

    plan: list[dict[str, object]] = []
    seen: set[str] = set()
    for lane in LANES:
        for task_id in TASKS_BY_LANE[lane]:
            if task_id in seen or task_id not in task_reasons:
                continue
            seen.add(task_id)
            plan.append(
                {
                    "id": task_id,
                    "reasons": list(dict.fromkeys(task_reasons[task_id])),
                }
            )
    return plan


def _read_changed_paths() -> list[str]:
    raw = sys.stdin.buffer.read()
    chunks = raw.split(b"\0") if b"\0" in raw else raw.splitlines()
    return [chunk.decode("utf-8", errors="strict") for chunk in chunks if chunk]


def write_github_output(path: Path, selected: dict[str, bool]) -> None:
    with path.open("a", encoding="utf-8") as output:
        for lane in LANES:
            output.write(f"{lane}={'true' if selected[lane] else 'false'}\n")


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser()
    subparsers = parser.add_subparsers(dest="command", required=True)

    head = subparsers.add_parser("assert-head")
    head.add_argument("--expected", required=True)
    head.add_argument("--repo", type=Path, default=Path.cwd())

    changes = subparsers.add_parser("classify")
    changes.add_argument("--github-output", type=Path)

    plan = subparsers.add_parser("plan")
    plan.add_argument("--json", action="store_true")
    return parser


def main() -> int:
    args = build_parser().parse_args()
    if args.command == "assert-head":
        return assert_exact_head(args.expected, args.repo)

    changed_paths = _read_changed_paths()

    if args.command == "plan":
        plan = build_task_plan(changed_paths)
        if args.json:
            print(json.dumps({"tasks": plan}, sort_keys=True, separators=(",", ":")))
        else:
            print("Required tasks:")
            for task in plan:
                print(f"  {task['id']}")
                for reason in task["reasons"]:
                    print(f"    - {reason}")
        return 0

    selected = classify(changed_paths)
    if args.github_output is not None:
        write_github_output(args.github_output, selected)
    print("Changed paths:")
    for changed_path in changed_paths:
        print(f"  {changed_path}")
    print("Behavioral lanes:")
    for lane in LANES:
        print(f"  {lane}: {'required' if selected[lane] else 'not required'}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
