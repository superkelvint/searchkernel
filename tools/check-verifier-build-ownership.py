#!/usr/bin/env python3
"""Fail closed if current-implementation Rust verifiers regain private Cargo workspaces."""

from __future__ import annotations

import json
import os
from pathlib import Path
import re
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]
REFERENCE = ROOT / "reference" / "vespa"


def target_spec(package: str, name: str, source: str, *owned_sources: str) -> dict[str, object]:
    return {
        "package": package,
        "name": name,
        "source": source,
        "owned_sources": (source, *owned_sources),
    }


ENGINE_EXAMPLES = (
    "searchkernel-m1-driver",
    "searchkernel-m2-driver",
    "searchkernel-m3-conformance-driver",
    "searchkernel-kstem-driver",
    "searchkernel-m4-driver",
    "searchkernel-m5-driver",
    "searchkernel-m6-driver",
    "searchkernel-m7-driver",
    "searchkernel-m7-error",
    "searchkernel-m8-driver",
    "searchkernel-m8-error",
    "searchkernel-m9-driver",
    "searchkernel-m9-error",
    "searchkernel-m10-driver",
    "searchkernel-m10-error",
    "searchkernel-m10-config-regressions",
    "searchkernel-searchreply-config",
)
SEARCHKERNEL_EXAMPLES = (
    "searchkernel-i-driver",
    "searchkernel-m11-driver",
    "searchkernel-m12-driver",
    "searchkernel-m12-adapter-driver",
    "searchkernel-m12-error",
    "searchkernel-m12-prepare-fixture",
    "searchkernel-m13-driver",
    "searchkernel-m13-adapter-driver",
    "searchkernel-m13-error",
    "searchkernel-m14-driver",
    "searchkernel-m14-adapter-driver",
    "searchkernel-m14-error",
    "searchkernel-recovery-driver",
)


def _engine_source(name: str) -> str:
    return f"core/crates/searchkernel-vespa-engine/examples/{name}.rs"


def _searchkernel_source(name: str) -> str:
    return f"core/crates/searchkernel/examples/{name}.rs"


MIGRATED_TARGETS = [
    *[
        target_spec(
            "searchkernel-vespa-engine",
            f"api-contract-m{n}",
            f"core/crates/searchkernel-vespa-engine/examples/api-contract-m{n}.rs",
        )
        for n in (1, 2, 3, 4, 6, 7, 8, 9, 10, 11, 12, 13, 14)
    ],
    *[target_spec("searchkernel-vespa-engine", name, _engine_source(name)) for name in ENGINE_EXAMPLES],
    *[target_spec("searchkernel", name, _searchkernel_source(name)) for name in SEARCHKERNEL_EXAMPLES],
]

for package, names, support in (
    ("searchkernel-vespa-engine", ("searchkernel-m8-driver", "searchkernel-m8-error"), "m8"),
    ("searchkernel-vespa-engine", ("searchkernel-m9-driver", "searchkernel-m9-error"), "m9"),
    (
        "searchkernel-vespa-engine",
        ("searchkernel-m10-driver", "searchkernel-m10-error", "searchkernel-m10-config-regressions"),
        "m10",
    ),
    (
        "searchkernel",
        (
            "searchkernel-m12-driver",
            "searchkernel-m12-adapter-driver",
            "searchkernel-m12-error",
            "searchkernel-m12-prepare-fixture",
        ),
        "m12",
    ),
    (
        "searchkernel",
        ("searchkernel-m13-driver", "searchkernel-m13-adapter-driver", "searchkernel-m13-error"),
        "m13",
    ),
    (
        "searchkernel",
        ("searchkernel-m14-driver", "searchkernel-m14-adapter-driver", "searchkernel-m14-error"),
        "m14",
    ),
):
    support_path = f"core/crates/{'searchkernel-vespa-engine' if package == 'searchkernel-vespa-engine' else 'searchkernel'}/examples/support/{support}.rs"
    for name in names:
        for spec in MIGRATED_TARGETS:
            if spec["package"] == package and spec["name"] == name:
                spec["owned_sources"] = (*spec["owned_sources"], support_path)
                break

MIGRATED_SOURCES = tuple(
    sorted({source for spec in MIGRATED_TARGETS for source in spec["owned_sources"]})
)

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

FOCUSED_BUILD_VERIFIERS = (
    REFERENCE / "m8/verify-m8.sh",
    REFERENCE / "m9/verify-m9.sh",
    REFERENCE / "m10/verify-m10.sh",
    REFERENCE / "m12/verify-m12.sh",
    REFERENCE / "m13/verify-m13.sh",
    REFERENCE / "m14/verify-m14.sh",
)


def fail(message: str) -> None:
    raise SystemExit(f"verifier build ownership: {message}")


def check_manifests_and_runners(root: Path = ROOT) -> None:
    reference = root / "reference" / "vespa"
    remaining = sorted(path.relative_to(root).as_posix() for path in reference.rglob("Cargo.toml"))
    if remaining:
        fail("standalone Cargo manifests remain under reference/vespa: " + ", ".join(remaining))
    old_manifests = [
        reference / "api-contract/Cargo.toml",
        *[reference / family / "rust-driver/Cargo.toml" for family in ("i", *[f"m{n}" for n in range(1, 15)], "recovery", "searchreply")],
    ]
    for manifest in old_manifests:
        if manifest.exists():
            fail(f"retired standalone verifier manifest remains: {manifest.relative_to(root)}")

    for script in reference.rglob("*.sh"):
        text = script.read_text(encoding="utf-8")
        if "--manifest-path" in text:
            fail(f"reference shell runner owns an explicit Cargo manifest path: {script.relative_to(root)}")
        if "rust-driver/Cargo.toml" in text or "api-contract/Cargo.toml" in text:
            fail(f"reference shell runner names a retired verifier workspace: {script.relative_to(root)}")
        if any(line.lstrip().startswith("rustc ") for line in text.splitlines()):
            fail(f"reference shell runner compiles Rust outside Cargo: {script.relative_to(root)}")
    private_trees = sorted(path for path in reference.rglob("rust-driver") if path.is_dir())
    if private_trees:
        fail("retired rust-driver source trees remain: " + ", ".join(str(p.relative_to(root)) for p in private_trees))
    retained_rust = sorted(path.relative_to(root).as_posix() for path in reference.rglob("*.rs"))
    if retained_rust:
        fail("active Rust verifier sources remain under reference/vespa: " + ", ".join(retained_rust))
    root_manifest = (root / "Cargo.toml").read_text(encoding="utf-8")
    if "reference/vespa/" in root_manifest:
        fail("root Cargo workspace still excludes or names a reference verifier tree")


def cargo_metadata(root: Path = ROOT) -> dict[str, object]:
    cargo = subprocess.run(
        ["bash", str(root / "tools/resolve-rust.sh"), "cargo"],
        cwd=root,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        check=False,
    )
    if cargo.returncode:
        fail(f"Rust Cargo resolver failed: {cargo.stderr.strip()}")
    completed = subprocess.run(
        [cargo.stdout.strip(), "metadata", "--format-version", "1", "--no-deps", "--manifest-path", str(root / "Cargo.toml")],
        cwd=root,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        check=False,
    )
    if completed.returncode:
        fail(f"cargo metadata failed: {completed.stderr.strip()}")
    try:
        value = json.loads(completed.stdout)
    except json.JSONDecodeError as error:
        fail(f"cargo metadata returned invalid JSON: {error}")
    if not isinstance(value, dict):
        fail("cargo metadata did not return an object")
    return value


def _metadata_targets(metadata: dict[str, object], root: Path) -> dict[tuple[str, str], dict[str, object]]:
    targets: dict[tuple[str, str], dict[str, object]] = {}
    for package in metadata.get("packages", []):
        if not isinstance(package, dict):
            continue
        package_name = package.get("name")
        if not isinstance(package_name, str):
            continue
        for target in package.get("targets", []):
            if isinstance(target, dict) and isinstance(target.get("name"), str):
                targets[(package_name, target["name"])] = target
    return targets


def _metadata_packages(metadata: dict[str, object]) -> dict[str, dict[str, object]]:
    return {
        package["name"]: package
        for package in metadata.get("packages", [])
        if isinstance(package, dict) and isinstance(package.get("name"), str)
    }


def check_cargo_target_ownership(
    root: Path,
    metadata: dict[str, object],
    specs: list[dict[str, object]] = MIGRATED_TARGETS,
    sources: tuple[str, ...] = MIGRATED_SOURCES,
) -> None:
    targets = _metadata_targets(metadata, root)
    packages = _metadata_packages(metadata)
    missing_sources = [source for source in sources if not (root / source).is_file()]
    if missing_sources:
        fail("migrated Rust verifier sources are missing: " + ", ".join(missing_sources))
    if sources is MIGRATED_SOURCES and len(MIGRATED_SOURCES) != 49:
        fail(f"structural target inventory expected 49 active Rust sources, found {len(MIGRATED_SOURCES)}")

    for spec in specs:
        package = spec["package"]
        name = spec["name"]
        package_metadata = packages.get(package)
        if package_metadata is None:
            fail(f"migrated Cargo package is not registered by metadata: {package!r}")
        manifest = Path(str(package_metadata.get("manifest_path", ""))).resolve()
        expected_manifest = (root / "core/crates" / package / "Cargo.toml").resolve()
        if manifest != expected_manifest:
            fail(
                f"Cargo metadata package {package!r} points to {manifest}, "
                f"not normal workspace manifest {expected_manifest}"
            )
        target = targets.get((package, name))
        if target is None:
            fail(f"migrated source target is not registered by Cargo metadata: {package!r} example {name!r}")
        if "example" not in target.get("kind", []):
            fail(f"migrated target is not a normal Cargo example target: {package!r} {name!r}")
        expected_source = (root / spec["source"]).resolve()
        actual_source = Path(str(target.get("src_path", ""))).resolve()
        if actual_source != expected_source:
            fail(
                f"Cargo target {package!r}/{name!r} owns {actual_source}, "
                f"not migrated source {expected_source}"
            )
        target_text = actual_source.read_text(encoding="utf-8")
        for source in spec["owned_sources"]:
            source_path = (root / source).resolve()
            if source_path == actual_source:
                continue
            relative = source_path.relative_to(actual_source.parent).as_posix()
            if f'#[path = "{relative}"]' not in target_text:
                fail(f"migrated source {source} is not referenced by Cargo target {package!r}/{name!r}")


def check_example_routing(root: Path = ROOT, metadata: dict[str, object] | None = None) -> None:
    metadata = metadata or cargo_metadata(root)
    targets = _metadata_targets(metadata, root)
    command_pattern = re.compile(
        r"\bcargo\b(?P<args>.*?)(?:\s*(?:&&|\|\||;|$))"
    )
    reference = root / "reference" / "vespa"
    for script in reference.rglob("*.sh"):
        text = re.sub(r"\\[ \t]*\n", " ", script.read_text(encoding="utf-8"))
        for line_number, line in enumerate(text.splitlines(), 1):
            if line.lstrip().startswith("#"):
                continue
            for match in command_pattern.finditer(line):
                args = match.group("args")
                if (
                    not re.search(r"(?:^|\s)--example(?:\s|$)", args)
                    or "-p" not in args
                ):
                    continue
                package_match = re.search(r"(?:^|\s)-p\s+([\w.-]+)", args)
                example_match = re.search(r"(?:^|\s)--example\s+([\w.-]+)", args)
                if not package_match or not example_match:
                    fail(f"could not parse Cargo package/example invocation: {script.relative_to(root)}:{line_number}")
                package, example = package_match.group(1), example_match.group(1)
                target = targets.get((package, example))
                if target is None:
                    fail(
                        f"{script.relative_to(root)}:{line_number} invokes Cargo example "
                        f"{example!r} that is not registered for package {package!r}"
                    )
                if "example" not in target.get("kind", []):
                    fail(f"{script.relative_to(root)}:{line_number} invokes non-example Cargo target {example!r}")


def check_focused_builds(root: Path = ROOT) -> None:
    for relative in (path.relative_to(ROOT) for path in FOCUSED_BUILD_VERIFIERS):
        script = root / relative
        text = re.sub(r"\\[ \t]*\n", " ", script.read_text(encoding="utf-8"))
        for line_number, line in enumerate(text.splitlines(), 1):
            if line.lstrip().startswith("#"):
                continue
            if re.search(r"\bcargo\b[^;&]*--examples\b", line):
                fail(f"focused verifier uses broad Cargo example build: {relative}:{line_number}")


def check_targets(root: Path = ROOT, metadata: dict[str, object] | None = None) -> None:
    check_cargo_target_ownership(root, metadata or cargo_metadata(root))
    targets = json.loads((root / "build/targets.json").read_text(encoding="utf-8"))
    registered = set(targets["verify"])
    environment = os.environ.copy()
    environment["SEARCHKERNEL_IN_CI_IMAGE"] = "1"
    completed = subprocess.run(
        [str(root / "dev"), "verify", "--list"],
        cwd=root,
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
    metadata = cargo_metadata()
    check_example_routing(metadata=metadata)
    check_focused_builds()
    check_targets(metadata=metadata)
    check_reference_and_oracle_preservation()
    check_dead_fixture_disposition()
    print("Rust verifier build ownership PASS (18 standalone workspaces removed; 49 sources in normal Cargo targets)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
