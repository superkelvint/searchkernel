#!/usr/bin/env python3
from __future__ import annotations

import argparse
import importlib.metadata
import re
from collections.abc import Mapping


EXACT_RUNTIME = {
    "flatbuffers": "25.12.19",
    "grpcio": "1.84.0",
}
PROTOBUF_MIN = (7, 35, 1)
PROTOBUF_MAX_EXCLUSIVE = (8, 0, 0)
CODEGEN_EXACT = {
    "protobuf": "7.35.1",
    "grpcio-tools": "1.84.0",
}


def parse_release(version: str) -> tuple[int, int, int] | None:
    match = re.match(r"^(\d+)\.(\d+)\.(\d+)", version)
    if match is None:
        return None
    return tuple(int(part) for part in match.groups())


def validate_versions(
    actual: Mapping[str, str | None],
    *,
    codegen: bool = False,
) -> list[str]:
    errors: list[str] = []

    for package, wanted in EXACT_RUNTIME.items():
        found = actual.get(package)
        if found != wanted:
            errors.append(f"{package} {found} (expected {wanted})")

    protobuf = actual.get("protobuf")
    release = parse_release(protobuf) if protobuf is not None else None
    if release is None:
        errors.append(
            f"protobuf {protobuf} (expected >=7.35.1,<8)"
        )
    elif not (PROTOBUF_MIN <= release < PROTOBUF_MAX_EXCLUSIVE):
        errors.append(
            f"protobuf {protobuf} (expected >=7.35.1,<8)"
        )

    if codegen:
        for package, wanted in CODEGEN_EXACT.items():
            found = actual.get(package)
            if found != wanted:
                errors.append(f"{package} {found} (expected {wanted} for codegen)")

    return errors


def installed_versions(*, codegen: bool = False) -> dict[str, str | None]:
    packages = set(EXACT_RUNTIME) | {"protobuf"}
    if codegen:
        packages.update(CODEGEN_EXACT)

    versions: dict[str, str | None] = {}
    for package in sorted(packages):
        try:
            versions[package] = importlib.metadata.version(package)
        except importlib.metadata.PackageNotFoundError:
            versions[package] = None
    return versions


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--codegen",
        action="store_true",
        help="require the exact Python gRPC/protobuf code-generation toolchain",
    )
    args = parser.parse_args()

    versions = installed_versions(codegen=args.codegen)
    errors = validate_versions(versions, codegen=args.codegen)
    if errors:
        for error in errors:
            print(error)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
