#!/usr/bin/env python3
"""Verify that every Search Kernel native ABI version authority agrees."""

from __future__ import annotations

import argparse
import json
import re
import subprocess
import sys
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]

SOURCE_HEADER = Path("core/native/vespa/searchkernel_vespa.h")
NATIVE_MANIFEST = Path("core/native/vespa/manifest.json")
RUST_ABI = Path("core/crates/searchkernel-vespa-bindings/src/abi.rs")
RUST_BINDINGS = Path("core/crates/searchkernel-vespa-bindings/src/bindings.rs")
REFERENCE_CONTRACT = Path("reference/vespa/NATIVE_ABI_CONTRACT.h")
PINNED_METADATA = Path("reference/vespa/PINNED_VESPA.json")
M0_NATIVE_OPEN_CLOSE_EXPECTED = Path(
    "reference/vespa/m0/expected/01-native-open-close.json"
)
M0_NATIVE_OPEN_CLOSE_SCENARIO = Path(
    "reference/vespa/m0/scenarios/01-native-open-close.json"
)
PHASE9_ADDITIVE_EXPECTED = Path(
    "reference/vespa/phase9/expected/01-additive-handles.json"
)

_HEADER_RE = re.compile(
    r"^\s*#\s*define\s+SK_NATIVE_ABI_VERSION\s+([0-9]+)U?\s*$",
    re.MULTILINE,
)
_RUST_EXPECTED_RE = re.compile(
    r"^\s*pub const EXPECTED_ABI: u32 = ([0-9]+);\s*$",
    re.MULTILINE,
)
_RUST_BINDINGS_RE = re.compile(
    r"^\s*pub const SK_NATIVE_ABI_VERSION: u32 = ([0-9]+);\s*$",
    re.MULTILINE,
)


class AuthorityCheckError(RuntimeError):
    pass


class RepositoryReader:
    def __init__(self, root: Path, *, git_index: bool = False) -> None:
        self.root = root.resolve()
        self.git_index = git_index

    def read_text(self, relative: Path) -> str:
        if self.git_index:
            completed = subprocess.run(
                ["git", "show", f":{relative.as_posix()}"],
                cwd=self.root,
                text=True,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                check=False,
            )
            if completed.returncode != 0:
                detail = completed.stderr.strip() or "path is absent from the index"
                raise AuthorityCheckError(
                    f"could not read staged {relative.as_posix()}: {detail}"
                )
            return completed.stdout

        path = self.root / relative
        try:
            return path.read_text(encoding="utf-8")
        except OSError as error:
            raise AuthorityCheckError(f"could not read {path}: {error}") from error


def _single_version(contents: str, pattern: re.Pattern[str], label: str) -> int:
    matches = pattern.findall(contents)
    if len(matches) != 1:
        raise AuthorityCheckError(
            f"expected exactly one native ABI version in {label}, found {len(matches)}"
        )
    return int(matches[0])


def _json_object(contents: str, label: str) -> dict[str, object]:
    try:
        value = json.loads(contents)
    except json.JSONDecodeError as error:
        raise AuthorityCheckError(f"invalid JSON in {label}: {error}") from error
    if not isinstance(value, dict):
        raise AuthorityCheckError(f"expected a JSON object in {label}")
    return value


def _json_int_field(
    value: dict[str, object], label: str, field: str
) -> int:
    version = value.get(field)
    if type(version) is not int:
        raise AuthorityCheckError(
            f"{label} must contain integer {field}, found {version!r}"
        )
    return version


def _json_version(value: dict[str, object], label: str) -> int:
    return _json_int_field(value, label, "native_abi_version")


def repository_authorities(
    reader: RepositoryReader,
) -> tuple[dict[str, int], str]:
    native_manifest = _json_object(
        reader.read_text(NATIVE_MANIFEST), NATIVE_MANIFEST.as_posix()
    )
    pinned = _json_object(
        reader.read_text(PINNED_METADATA), PINNED_METADATA.as_posix()
    )
    m0_expected = _json_object(
        reader.read_text(M0_NATIVE_OPEN_CLOSE_EXPECTED),
        M0_NATIVE_OPEN_CLOSE_EXPECTED.as_posix(),
    )
    m0_scenario = _json_object(
        reader.read_text(M0_NATIVE_OPEN_CLOSE_SCENARIO),
        M0_NATIVE_OPEN_CLOSE_SCENARIO.as_posix(),
    )
    phase9_expected = _json_object(
        reader.read_text(PHASE9_ADDITIVE_EXPECTED),
        PHASE9_ADDITIVE_EXPECTED.as_posix(),
    )

    pin_commit = pinned.get("commit")
    if not isinstance(pin_commit, str) or not pin_commit:
        raise AuthorityCheckError(
            f"{PINNED_METADATA.as_posix()} must contain a non-empty commit"
        )
    native_commit = native_manifest.get("vespa_commit")
    if native_commit != pin_commit:
        raise AuthorityCheckError(
            f"native manifest Vespa commit {native_commit!r} does not match "
            f"pinned commit {pin_commit!r}"
        )

    authorities = {
        "source header": _single_version(
            reader.read_text(SOURCE_HEADER),
            _HEADER_RE,
            SOURCE_HEADER.as_posix(),
        ),
        "native manifest": _json_version(
            native_manifest, NATIVE_MANIFEST.as_posix()
        ),
        "Rust EXPECTED_ABI": _single_version(
            reader.read_text(RUST_ABI),
            _RUST_EXPECTED_RE,
            RUST_ABI.as_posix(),
        ),
        "generated Rust bindings": _single_version(
            reader.read_text(RUST_BINDINGS),
            _RUST_BINDINGS_RE,
            RUST_BINDINGS.as_posix(),
        ),
        "reference ABI contract": _single_version(
            reader.read_text(REFERENCE_CONTRACT),
            _HEADER_RE,
            REFERENCE_CONTRACT.as_posix(),
        ),
        "pinned metadata": _json_version(
            pinned, PINNED_METADATA.as_posix()
        ),
        "M0 native-open-close expected": _json_int_field(
            m0_expected,
            M0_NATIVE_OPEN_CLOSE_EXPECTED.as_posix(),
            "abi_version",
        ),
        "M0 native-open-close scenario": _json_int_field(
            m0_scenario,
            M0_NATIVE_OPEN_CLOSE_SCENARIO.as_posix(),
            "native_abi_version",
        ),
        "Phase 9 additive expected": _json_int_field(
            phase9_expected,
            PHASE9_ADDITIVE_EXPECTED.as_posix(),
            "abi",
        ),
    }
    return authorities, pin_commit


def _read_external_text(path: Path) -> str:
    try:
        return path.read_text(encoding="utf-8")
    except OSError as error:
        raise AuthorityCheckError(f"could not read {path}: {error}") from error


def add_sdk_authorities(
    authorities: dict[str, int],
    *,
    sdk_header: Path,
    sdk_manifest: Path,
    pinned_commit: str,
) -> None:
    authorities["SDK header"] = _single_version(
        _read_external_text(sdk_header),
        _HEADER_RE,
        str(sdk_header),
    )
    manifest = _json_object(_read_external_text(sdk_manifest), str(sdk_manifest))
    authorities["SDK manifest"] = _json_version(manifest, str(sdk_manifest))
    sdk_commit = manifest.get("vespa_commit")
    if sdk_commit != pinned_commit:
        raise AuthorityCheckError(
            f"SDK manifest Vespa commit {sdk_commit!r} does not match "
            f"pinned commit {pinned_commit!r}"
        )


def disagreement(authorities: dict[str, int]) -> str | None:
    expected = authorities["source header"]
    if all(value == expected for value in authorities.values()):
        return None
    details = ", ".join(f"{name}={value}" for name, value in authorities.items())
    return f"native ABI authorities disagree: {details}"


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        description="check Search Kernel native ABI version authorities"
    )
    parser.add_argument("--root", type=Path, default=ROOT)
    parser.add_argument(
        "--git-index",
        action="store_true",
        help="read checked-in authorities from the Git index instead of the working tree",
    )
    parser.add_argument("--sdk-header", type=Path)
    parser.add_argument("--sdk-manifest", type=Path)
    args = parser.parse_args(argv)

    if (args.sdk_header is None) != (args.sdk_manifest is None):
        parser.error("--sdk-header and --sdk-manifest must be supplied together")

    try:
        authorities, pinned_commit = repository_authorities(
            RepositoryReader(args.root, git_index=args.git_index)
        )
        if args.sdk_header is not None and args.sdk_manifest is not None:
            add_sdk_authorities(
                authorities,
                sdk_header=args.sdk_header,
                sdk_manifest=args.sdk_manifest,
                pinned_commit=pinned_commit,
            )
        mismatch = disagreement(authorities)
    except AuthorityCheckError as error:
        print(f"native ABI authority check ERROR: {error}", file=sys.stderr)
        return 2

    if mismatch is not None:
        print(f"native ABI authority check FAILED: {mismatch}", file=sys.stderr)
        return 1

    print(
        "native ABI authority check PASS: "
        f"all {len(authorities)} authorities report "
        f"{authorities['source header']}"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
