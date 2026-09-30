#!/usr/bin/env python3
from __future__ import annotations

import json
import shlex
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]

EXPECTED_SUPPORTED = {
    ("rust-embedded", "native", "native-full"),
    ("searchkerneld", "native", "client-e2e"),
    ("python", "managed-local", "native-full"),
    ("python", "connect", "client-e2e"),
    ("typescript-node", "connect", "client-e2e"),
    ("ruby", "connect", "client-e2e"),
}

REQUIRED_DEFERRED = {
    "typescript-browser-grpc-web",
    "php",
    "ruby-managed-local",
    "go",
    "macos",
    "windows",
    "linux-aarch64",
}

PACKAGED_QUALIFICATION = "tools/qualify-consumer-release.py dist/release"
PACKAGED_SURFACES = {"rust-embedded", "searchkerneld", "python", "typescript-node"}

REQUIRED_VERIFICATION = {
    ("rust-embedded", "native"): "./dev verify persistence",
    ("searchkerneld", "native"): "./dev verify product.integration",
    ("python", "managed-local"): "./dev test python",
    ("python", "connect"): "./dev test python",
    ("typescript-node", "connect"): "./dev test typescript",
    ("ruby", "connect"): "./dev test ruby",
}


def _load_json(path: Path):
    return json.loads(path.read_text(encoding="utf-8"))


def validate_verification_command(root: Path, command: str, registry: dict) -> None:
    try:
        argv = shlex.split(command)
    except ValueError as error:
        raise SystemExit(f"invalid release verification command {command!r}: {error}") from error

    if len(argv) != 3 or argv[0] != "./dev" or argv[1] not in {"test", "verify"}:
        raise SystemExit(
            "release verification must use one canonical './dev test|verify <target>' command: "
            f"{command}"
        )

    dev = root / "dev"
    if not dev.is_file():
        raise SystemExit("canonical ./dev entrypoint is missing")

    target_kind, target = argv[1], argv[2]
    registered = registry.get(target_kind)
    if not isinstance(registered, dict) or target not in registered:
        raise SystemExit(
            f"release verification references unregistered {target_kind} target: {target}"
        )


def validate(root: Path = ROOT) -> None:
    authority = _load_json(root / "docs" / "release-support.json")
    if authority.get("platform") != {
        "os": "linux",
        "arch": "x86_64",
        "libc": "glibc",
    }:
        raise SystemExit("release platform must remain explicit Linux x86_64 glibc")

    actual = {
        (row["surface"], row["runtime"], row["evidence"])
        for row in authority.get("supported", [])
    }
    if actual != EXPECTED_SUPPORTED:
        raise SystemExit(f"supported release matrix drifted: {sorted(actual)}")

    deferred = set(authority.get("deferred", []))
    if not REQUIRED_DEFERRED <= deferred:
        raise SystemExit("unsupported/deferred release surfaces are incomplete")

    registry = _load_json(root / "build" / "targets.json")
    if registry.get("schema_version") != 2:
        raise SystemExit("unsupported build target registry schema")

    for row in authority["supported"]:
        key = (row["surface"], row["runtime"])
        command = row.get("verification")
        expected = REQUIRED_VERIFICATION.get(key)
        if command != expected:
            raise SystemExit(
                f"{key} has no authoritative executable verification command: "
                f"expected {expected!r}, found {command!r}"
            )
        validate_verification_command(root, command, registry)

        if row["surface"] in PACKAGED_SURFACES:
            if row.get("qualification") != PACKAGED_QUALIFICATION:
                raise SystemExit(
                    f"{key} is missing packaged clean-machine qualification ownership"
                )

    readme = (root / "README.md").read_text(encoding="utf-8")
    clients = (root / "clients" / "README.md").read_text(encoding="utf-8")
    support = (root / "docs" / "release-support.md").read_text(encoding="utf-8")

    if "docs/release-support.md" not in readme:
        raise SystemExit("README.md does not link the authoritative release support matrix")
    if "../docs/release-support.md" not in clients:
        raise SystemExit("clients/README.md does not link the authoritative release support matrix")
    if "Ruby (connect-to-daemon)" not in readme:
        raise SystemExit("README.md must describe Ruby as connect-to-daemon release support")
    if "Ruby (connect-to-daemon)" not in clients:
        raise SystemExit("clients/README.md must describe Ruby as connect-to-daemon release support")
    if "| Ruby | Supported | connect-to-daemon |" not in support:
        raise SystemExit("release-support.md must keep Ruby connect-only")
    if "| Ruby managed-local | Deferred |" not in support:
        raise SystemExit("release-support.md must keep Ruby managed-local deferred")

    print(
        "Release support matrix PASS: Linux x86_64 glibc "
        "Rust/daemon/Python/Node plus Ruby connect-only scope is explicit and qualified"
    )


if __name__ == "__main__":
    validate()
