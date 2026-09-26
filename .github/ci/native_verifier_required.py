#!/usr/bin/env python3
"""Classify whether a PR change requires the native verifier lane.

This policy is intentionally independent of verification/dag.toml while DAG
authorization is suspended. Keep it conservative: false positives cost runner
time; false negatives can merge native regressions.
"""

from __future__ import annotations

import fnmatch
import sys
from collections.abc import Iterable

NATIVE_PATTERNS = (
    "api/searchkernel.fbs",
    "api/searchkernel_request.fbs",
    "api/searchkernel_response.fbs",
    "Cargo.toml",
    "Cargo.lock",
    ".cargo/**",
    "rust-toolchain*",
    "core/crates/searchkernel-ir-adapter/**",
    "core/crates/searchkernel-vespa-bindings/**",
    "core/crates/searchkernel-vespa-engine/**",
    "core/crates/searchkernel/Cargo.toml",
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
    "core/native/**",
    "searchkerneld/**",
    "reference/persistence/**",
    "reference/vespa/**",
    "tools/check-native-*.sh",
    "tools/check-native-*.py",
    "tools/check-native-bindings/**",
    "tools/check-vespa-bindings.sh",
    "tools/check_abi_surface.py",
    "tools/test_check_abi_surface.py",
    "tools/test_check_native_*.py",
    "tools/test-native-*.sh",
    "tools/test-check-native-abi.sh",
    "tools/native-*.py",
    "tools/native-contract-tests/**",
    "tools/*native-sdk*.sh",
    "tools/*native-base*.sh",
    "tools/*shared-library*.sh",
    "tools/copy-shared-library-into-sdk.sh",
    "tools/install-native-base-artifact.sh",
    "tools/package-native-base-artifact.sh",
    "tools/resolve-native-sdk.sh",
    "tools/coverage-native-adapter.sh",
    "tools/coverage-m0-probe.sh",
    "tools/m0-probe-abi-double.c",
    "tools/check-persistence-*.sh",
    "tools/run-python-native-e2e.sh",
    "tools/actions-runner-setup.sh",
    ".github/workflows/native-verifier-runner.yml",
    ".github/workflows/ci-image-runner.yml",
    ".github/workflows/behavioral-verification.yml",
    ".github/ci/native_verifier_required.py",
    ".github/ci/test_native_verifier_required.py",
    ".github/ci/test_native_verifier_runner.py",
)


def matching_rule(path: str) -> str | None:
    normalized = path.strip().replace("\\", "/")
    if not normalized:
        return None
    for pattern in NATIVE_PATTERNS:
        if fnmatch.fnmatchcase(normalized, pattern):
            return pattern
    return None


def classify(paths: Iterable[str]) -> list[tuple[str, str]]:
    matches: list[tuple[str, str]] = []
    for raw_path in paths:
        path = raw_path.strip().replace("\\", "/")
        rule = matching_rule(path)
        if rule is not None:
            matches.append((path, rule))
    return matches


def main() -> int:
    paths = [line for line in sys.stdin.read().splitlines() if line.strip()]
    matches = classify(paths)
    for path, rule in matches:
        print(f"native verifier required: {path} (rule {rule})", file=sys.stderr)
    print("true" if matches else "false")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
