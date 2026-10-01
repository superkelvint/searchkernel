#!/usr/bin/env python3
"""Validate that a native-base lib directory is self-contained for non-baseline DSOs."""

from __future__ import annotations

import json
import pathlib
import shutil
import sys

from native_runtime_closure import (
    SYSTEM_BASELINE,
    is_elf,
    ldd_resolutions,
    elf_soname,
    needed_libraries,
    resolve_origin_dependency,
    sha256_file,
)

def die(message: str) -> "NoReturn":
    raise SystemExit(message)


def validate(
    libdir: pathlib.Path,
    extra_elfs: list[pathlib.Path],
    require_provenance: bool = False,
) -> None:
    if not libdir.is_dir():
        die("native runtime lib directory does not exist: {}".format(libdir))

    readelf = shutil.which("readelf")
    ldd = shutil.which("ldd")
    if readelf is None or ldd is None:
        die("native runtime closure validation requires readelf and ldd")

    root = libdir.resolve()
    payloads: dict[pathlib.Path, set[str]] = {}

    for entry in libdir.iterdir():
        try:
            resolved = entry.resolve(strict=True)
        except OSError as exc:
            die("broken native runtime entry {}: {}".format(entry, exc))

        try:
            resolved.relative_to(root)
        except ValueError:
            die(
                "native runtime entry escapes bundled lib directory: {} -> {}".format(
                    entry, resolved
                )
            )

        if not resolved.is_file() or not is_elf(resolved):
            continue
        payloads.setdefault(resolved, set()).add(entry.name)

    if not payloads:
        die("native runtime lib directory contains no ELF shared libraries: {}".format(libdir))

    sonames: dict[str, tuple[str, pathlib.Path]] = {}
    for payload in payloads:
        soname = elf_soname(payload, readelf)
        if soname is None:
            continue
        digest = sha256_file(payload)
        previous = sonames.get(soname)
        if previous is not None and previous[0] != digest:
            die(
                f"conflicting packaged ELF candidates provide SONAME {soname}: "
                f"{previous[1]} vs {payload}"
            )
        sonames[soname] = (digest, payload)

    provenance_path = libdir / ".searchkernel-runtime-provenance.json"
    if provenance_path.exists() or require_provenance:
        try:
            provenance = json.loads(provenance_path.read_text(encoding="utf-8"))
        except (OSError, json.JSONDecodeError) as exc:
            die(f"invalid runtime provenance {provenance_path}: {exc}")
        if provenance.get("format_version") != 1 or not isinstance(provenance.get("libraries"), list):
            die(f"unsupported runtime provenance format in {provenance_path}")
        recorded: set[pathlib.Path] = set()
        for item in provenance["libraries"]:
            if not isinstance(item, dict):
                die(f"invalid runtime provenance entry in {provenance_path}")
            name = item.get("packaged_name")
            source = item.get("source_identity")
            expected_hash = item.get("sha256")
            required_by = item.get("required_by")
            if (
                not isinstance(name, str)
                or pathlib.Path(name).name != name
                or not isinstance(source, str)
                or not source
                or pathlib.Path(source).is_absolute()
                or not isinstance(expected_hash, str)
                or len(expected_hash) != 64
                or not isinstance(required_by, list)
                or required_by != sorted(set(required_by))
            ):
                die(f"invalid runtime provenance fields for {name!r} in {provenance_path}")
            entry = libdir / name
            try:
                payload = entry.resolve(strict=True)
            except (OSError, RuntimeError) as exc:
                die(f"runtime provenance names missing or broken packaged ELF {entry}: {exc}")
            if not payload.is_file() or not is_elf(payload):
                die(f"runtime provenance names a non-ELF packaged file: {entry}")
            try:
                payload.relative_to(root)
            except ValueError:
                die(f"runtime provenance packaged file escapes bundle: {entry} -> {payload}")
            if sha256_file(payload) != expected_hash:
                die(f"runtime provenance SHA-256 mismatch for {entry}")
            if elf_soname(payload, readelf) != item.get("soname"):
                die(f"runtime provenance SONAME mismatch for {entry}")
            recorded.add(payload)
        if recorded != set(payloads):
            missing_records = sorted(path.name for path in set(payloads) - recorded)
            extra_records = sorted(path.name for path in recorded - set(payloads))
            die(
                "runtime provenance does not describe the packaged ELF inventory "
                f"(missing={missing_records}, extra={extra_records})"
            )

    inspected = set(payloads)
    extra_roots: list[pathlib.Path] = []
    for extra in extra_elfs:
        try:
            resolved = extra.resolve(strict=True)
        except (OSError, RuntimeError) as exc:
            die("invalid extra native ELF {}: {}".format(extra, exc))
        if not resolved.is_file() or not is_elf(resolved):
            die("extra native runtime input is not an ELF file: {}".format(extra))
        inspected.add(resolved)
        extra_roots.append(resolved.parent)

    invalid: list[str] = []
    for payload in sorted(inspected, key=lambda item: str(item)):
        try:
            needed_names = needed_libraries(payload, readelf)
        except RuntimeError as exc:
            die(str(exc))
        try:
            resolved_libraries = ldd_resolutions(payload, ldd, [root])
        except RuntimeError as exc:
            invalid.append(str(exc))
            continue
        for needed in sorted(needed_names):
            if needed.startswith(("$ORIGIN/", "${ORIGIN}/")):
                origin_dependency = resolve_origin_dependency(payload, needed)
                if origin_dependency is None:
                    invalid.append(f"{payload.name}: unresolved origin-relative dependency {needed}")
                    continue
                try:
                    origin_dependency.relative_to(root)
                except ValueError:
                    if not any(origin_dependency.is_relative_to(extra_root) for extra_root in extra_roots):
                        invalid.append(
                            f"{payload.name}: origin-relative dependency escapes allowed package roots: "
                            f"{needed} -> {origin_dependency}"
                        )
                continue
            if "/" in needed:
                invalid.append(f"{payload.name}: unsupported absolute/path DT_NEEDED entry {needed}")
                continue
            resolved = resolved_libraries.get(needed)
            if resolved is None:
                invalid.append(f"{payload.name}: loader did not resolve DT_NEEDED {needed}")
                continue
            try:
                resolved_real = resolved.resolve(strict=True)
            except (OSError, RuntimeError) as exc:
                invalid.append(f"{payload.name}: broken loader resolution for {needed}: {exc}")
                continue
            try:
                resolved_real.relative_to(root)
                inside_bundle = True
            except ValueError:
                inside_bundle = False
            if not inside_bundle and needed not in SYSTEM_BASELINE:
                invalid.append(
                    f"{payload.name}: non-baseline dependency {needed} resolved outside bundle: {resolved_real}"
                )
            if inside_bundle and needed not in SYSTEM_BASELINE:
                actual_soname = elf_soname(resolved_real, readelf)
                if actual_soname != needed:
                    invalid.append(
                        f"{payload.name}: DT_NEEDED {needed} resolved to ELF with "
                        f"SONAME {actual_soname!r}: {resolved_real}"
                    )

    if invalid:
        die(
            "native runtime dependency closure is incompatible or incomplete:\n  "
            + "\n  ".join(invalid)
        )


def main() -> int:
    if len(sys.argv) < 2:
        print(
            "usage: tools/check-native-runtime-closure.py <lib-directory> [extra-elf ...] [--require-provenance]",
            file=sys.stderr,
        )
        return 2

    args = sys.argv[2:]
    require_provenance = "--require-provenance" in args
    args = [value for value in args if value != "--require-provenance"]
    validate(pathlib.Path(sys.argv[1]), [pathlib.Path(value) for value in args], require_provenance)
    print("native runtime dependency closure: PASS")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
