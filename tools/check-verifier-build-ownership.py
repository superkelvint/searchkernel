#!/usr/bin/env python3
"""Fail closed if current-implementation Rust verifiers regain private Cargo workspaces."""

from __future__ import annotations

import json
import os
from pathlib import Path
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]
REFERENCE = ROOT / "reference" / "vespa"

API_EXAMPLES = [
    ROOT / "core/crates/searchkernel-vespa-engine/examples" / f"api-contract-m{n}.rs"
    for n in (1, 2, 3, 4, 6, 7, 8, 9, 10, 11, 12, 13, 14)
]
ENGINE_EXAMPLES = [
    "searchkernel-m1-driver.rs",
    "searchkernel-m2-driver.rs",
    "searchkernel-m3-conformance-driver.rs",
    "searchkernel-kstem-driver.rs",
    "searchkernel-m4-driver.rs",
    "searchkernel-m5-driver.rs",
    "searchkernel-m6-driver.rs",
    "searchkernel-m7-driver.rs",
    "searchkernel-m7-error.rs",
    "searchkernel-m8-driver.rs",
    "searchkernel-m8-error.rs",
    "searchkernel-m9-driver.rs",
    "searchkernel-m9-error.rs",
    "searchkernel-m10-driver.rs",
    "searchkernel-m10-error.rs",
    "searchkernel-m10-config-regressions.rs",
    "searchkernel-searchreply-config.rs",
]
SEARCHKERNEL_EXAMPLES = [
    "searchkernel-i-driver.rs",
    "searchkernel-m11-driver.rs",
    "searchkernel-m12-driver.rs",
    "searchkernel-m12-adapter-driver.rs",
    "searchkernel-m12-error.rs",
    "searchkernel-m12-prepare-fixture.rs",
    "searchkernel-m13-driver.rs",
    "searchkernel-m13-adapter-driver.rs",
    "searchkernel-m13-error.rs",
    "searchkernel-m14-driver.rs",
    "searchkernel-m14-adapter-driver.rs",
    "searchkernel-m14-error.rs",
    "searchkernel-recovery-driver.rs",
]
MIGRATED_SOURCES = (
    API_EXAMPLES
    + [ROOT / "core/crates/searchkernel-vespa-engine/examples" / name for name in ENGINE_EXAMPLES]
    + [ROOT / "core/crates/searchkernel/examples" / name for name in SEARCHKERNEL_EXAMPLES]
    + [
        ROOT / "core/crates/searchkernel-vespa-engine/examples/support" / f"m{n}.rs"
        for n in (8, 9, 10)
    ]
    + [
        ROOT / "core/crates/searchkernel/examples/support" / f"m{n}.rs"
        for n in (12, 13, 14)
    ]
)

EXPECTED_OLD_MANIFESTS = [
    REFERENCE / "api-contract/Cargo.toml",
    *[REFERENCE / family / "rust-driver/Cargo.toml" for family in ("i", *[f"m{n}" for n in range(1, 15)], "recovery", "searchreply")],
]
ORACLE_ROOTS = (
    "m2/oracle",
    "m3/oracle",
    "m3/kstem/oracle",
    "m4/oracle",
    "m6/oracle",
    "m7/oracle",
    "m8/oracle",
    "m9/oracle",
    "m9-general-grouping-verifier/oracle",
    "m10/oracle",
    "m12/oracle",
    "m13/oracle",
    "m14/oracle",
    "searchreply/oracle",
)


def fail(message: str) -> None:
    raise SystemExit(f"verifier build ownership: {message}")


def check_manifests_and_runners() -> None:
    remaining = sorted(path.relative_to(ROOT).as_posix() for path in REFERENCE.rglob("Cargo.toml"))
    if remaining:
        fail("standalone Cargo manifests remain under reference/vespa: " + ", ".join(remaining))
    for manifest in EXPECTED_OLD_MANIFESTS:
        if manifest.exists():
            fail(f"retired standalone verifier manifest remains: {manifest.relative_to(ROOT)}")

    for script in REFERENCE.rglob("*.sh"):
        text = script.read_text(encoding="utf-8")
        if "--manifest-path" in text:
            fail(f"reference shell runner owns an explicit Cargo manifest path: {script.relative_to(ROOT)}")
        if "rust-driver/Cargo.toml" in text or "api-contract/Cargo.toml" in text:
            fail(f"reference shell runner names a retired verifier workspace: {script.relative_to(ROOT)}")
        if any(line.lstrip().startswith("rustc ") for line in text.splitlines()):
            fail(f"reference shell runner compiles Rust outside Cargo: {script.relative_to(ROOT)}")
    private_trees = sorted(path for path in REFERENCE.rglob("rust-driver") if path.is_dir())
    if private_trees:
        fail("retired rust-driver source trees remain: " + ", ".join(str(p.relative_to(ROOT)) for p in private_trees))
    root_manifest = (ROOT / "Cargo.toml").read_text(encoding="utf-8")
    if "reference/vespa/" in root_manifest:
        fail("root Cargo workspace still excludes or names a reference verifier tree")


def check_targets() -> None:
    missing = [path.relative_to(ROOT).as_posix() for path in MIGRATED_SOURCES if not path.is_file()]
    if missing:
        fail("migrated Cargo example sources are missing: " + ", ".join(missing))
    if len(MIGRATED_SOURCES) != 49:
        fail(f"structural target inventory expected 49 active Rust sources, found {len(MIGRATED_SOURCES)}")

    targets = json.loads((ROOT / "build/targets.json").read_text(encoding="utf-8"))
    registered = set(targets["verify"])
    environment = os.environ.copy()
    environment["SEARCHKERNEL_IN_CI_IMAGE"] = "1"
    completed = subprocess.run(
        [str(ROOT / "dev"), "verify", "--list"],
        cwd=ROOT,
        env=environment,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        check=False,
    )
    if completed.returncode:
        fail("./dev verify --list failed: " + completed.stdout.strip())
    resolved = {line.strip() for line in completed.stdout.splitlines() if line.strip() in registered}
    if resolved != registered:
        missing_targets = sorted(registered - resolved)
        fail("registered ./dev verify targets do not resolve: " + ", ".join(missing_targets))


def check_reference_and_oracle_preservation() -> None:
    required = [
        REFERENCE / "PINNED_VESPA.json",
        REFERENCE / "m1/scenarios",
        REFERENCE / "m1/expected",
        REFERENCE / "m5/fixtures/persisted-v1/index",
        REFERENCE / "m14/scenarios",
        REFERENCE / "m14/expected",
        REFERENCE / "searchreply/scenario.json",
        REFERENCE / "searchreply/expected/searchreply.json",
        REFERENCE / "recovery/CONTRACT.md",
    ]
    missing = [path.relative_to(ROOT).as_posix() for path in required if not path.exists()]
    if missing:
        fail("frozen reference material is missing: " + ", ".join(missing))

    for relative in ORACLE_ROOTS:
        oracle = REFERENCE / relative
        if not oracle.is_dir() or not any(p.suffix in {".cc", ".cpp", ".java"} for p in oracle.rglob("*")):
            fail(f"independent direct oracle source is missing: reference/vespa/{relative}")

    for path in [*ROOT.glob("core/crates/**/Cargo.toml"), *ROOT.glob("core/crates/**/*.rs")]:
        source = path.read_text(encoding="utf-8", errors="replace")
        if "reference/vespa/" in source and "/oracle" in source:
            fail(f"Search Kernel Cargo source links a reference oracle: {path.relative_to(ROOT)}")


def check_dead_fixture_disposition() -> None:
    dead_fixture = REFERENCE / "api-contract/src/bin/m-api.rs"
    if dead_fixture.exists():
        fail("m-api.rs remains despite having no active supported caller")
    active_roots = [REFERENCE, ROOT / "build/targets.json", ROOT / "dev", ROOT / ".github/ci"]
    active_extensions = {".sh", ".py", ".json", ".rs", ".toml", ".yml", ".yaml"}
    for base in active_roots:
        candidates = [base] if base.is_file() else base.rglob("*")
        for path in candidates:
            if not path.is_file() or path.suffix not in active_extensions:
                continue
            if path.name in {"README.md", "CONTRACT.md", "compiled-sources.md"}:
                continue
            text = path.read_text(encoding="utf-8", errors="replace")
            if "m-api.rs" in text or "api-contract/src/bin/m-api" in text:
                fail(f"active supported caller still names m-api.rs: {path.relative_to(ROOT)}")


def main() -> int:
    check_manifests_and_runners()
    check_targets()
    check_reference_and_oracle_preservation()
    check_dead_fixture_disposition()
    print("Rust verifier build ownership PASS (18 standalone workspaces removed; 49 sources in normal Cargo targets)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
