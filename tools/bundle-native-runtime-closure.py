#!/usr/bin/env python3
"""Bundle non-baseline ELF DT_NEEDED closure from the trusted producer host."""

from __future__ import annotations

import argparse
import os
import pathlib
import re
import shutil
import subprocess
import sys

from native_runtime_closure import SYSTEM_BASELINE, is_elf, needed_libraries


LDD_ARROW_RE = re.compile(r"^\s*(?P<name>\S+)\s+=>\s+(?P<path>/\S+)")
LDD_DIRECT_RE = re.compile(r"^\s*(?P<path>/\S+)\s+\(")


def die(message: str) -> "NoReturn":
    raise SystemExit(message)


def bundle_entries(libdir: pathlib.Path) -> tuple[set[str], list[pathlib.Path]]:
    root = libdir.resolve()
    available: set[str] = set()
    payloads: set[pathlib.Path] = set()

    for entry in libdir.iterdir():
        try:
            resolved = entry.resolve(strict=True)
        except OSError as exc:
            die(f"broken native runtime entry {entry}: {exc}")
        try:
            resolved.relative_to(root)
        except ValueError:
            die(f"native runtime entry escapes bundled lib directory: {entry} -> {resolved}")
        if not resolved.is_file() or not is_elf(resolved):
            continue
        available.add(entry.name)
        payloads.add(resolved)

    if not payloads:
        die(f"native runtime lib directory contains no ELF shared libraries: {libdir}")
    return available, sorted(payloads, key=str)


def ldd_resolution(
    payload: pathlib.Path,
    needed: str,
    libdir: pathlib.Path,
    search_dirs: list[pathlib.Path],
    ldd: str,
) -> pathlib.Path | None:
    env = os.environ.copy()
    env.pop("LD_PRELOAD", None)
    paths = [str(libdir), *(str(path) for path in search_dirs)]
    inherited = env.get("LD_LIBRARY_PATH")
    if inherited:
        paths.append(inherited)
    env["LD_LIBRARY_PATH"] = ":".join(paths)

    completed = subprocess.run(
        [ldd, str(payload)],
        check=False,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        env=env,
    )
    for raw in completed.stdout.splitlines():
        match = LDD_ARROW_RE.match(raw)
        if match is not None and match.group("name") == needed:
            return pathlib.Path(match.group("path"))
        match = LDD_DIRECT_RE.match(raw)
        if match is not None:
            candidate = pathlib.Path(match.group("path"))
            if candidate.name == needed:
                return candidate
    return None


def bundle(
    libdir: pathlib.Path,
    search_dirs: list[pathlib.Path],
    extra_elfs: list[pathlib.Path],
) -> int:
    if not libdir.is_dir():
        die(f"native runtime lib directory does not exist: {libdir}")

    readelf = shutil.which("readelf")
    ldd = shutil.which("ldd")
    if readelf is None:
        die("native runtime bundling requires readelf")
    if ldd is None:
        die("native runtime bundling requires ldd")

    root = libdir.resolve()
    resolved_search_dirs: list[pathlib.Path] = []
    for directory in search_dirs:
        try:
            resolved = directory.resolve(strict=True)
        except OSError as exc:
            die(f"runtime search directory is unavailable: {directory}: {exc}")
        if not resolved.is_dir():
            die(f"runtime search path is not a directory: {resolved}")
        resolved_search_dirs.append(resolved)

    available, queue = bundle_entries(libdir)
    for extra in extra_elfs:
        try:
            resolved_extra = extra.resolve(strict=True)
        except OSError as exc:
            die(f"invalid extra native ELF {extra}: {exc}")
        if not resolved_extra.is_file() or not is_elf(resolved_extra):
            die(f"extra native runtime input is not an ELF file: {resolved_extra}")
        queue.append(resolved_extra)

    inspected: set[pathlib.Path] = set()
    copied = 0

    while queue:
        payload = queue.pop(0)
        resolved_payload = payload.resolve(strict=True)
        if resolved_payload in inspected:
            continue
        inspected.add(resolved_payload)

        try:
            needed_names = needed_libraries(resolved_payload, readelf)
        except RuntimeError as exc:
            die(str(exc))

        for needed in sorted(needed_names):
            if needed in SYSTEM_BASELINE or needed in available:
                continue

            source = ldd_resolution(
                resolved_payload,
                needed,
                root,
                resolved_search_dirs,
                ldd,
            )
            if source is None:
                die(
                    f"{resolved_payload.name}: could not resolve non-baseline "
                    f"runtime dependency {needed} from the trusted producer environment"
                )
            try:
                source = source.resolve(strict=True)
            except OSError as exc:
                die(f"{resolved_payload.name}: invalid runtime dependency {needed}: {exc}")
            if not source.is_file() or not is_elf(source):
                die(
                    f"{resolved_payload.name}: resolved runtime dependency "
                    f"{needed} is not an ELF file: {source}"
                )

            destination = root / needed
            if destination.exists() or destination.is_symlink():
                die(
                    f"{resolved_payload.name}: unresolved dependency {needed} "
                    "collides with a non-usable bundled entry"
                )

            shutil.copy2(source, destination)
            available.add(needed)
            queue.append(destination)
            copied += 1
            print(
                f"bundled runtime dependency {needed} from {source}",
                file=sys.stderr,
            )

    return copied


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("libdir", type=pathlib.Path)
    parser.add_argument(
        "--search-dir",
        action="append",
        default=[],
        type=pathlib.Path,
        help="additional trusted producer library directory used for ldd resolution",
    )
    parser.add_argument(
        "--extra-elf",
        action="append",
        default=[],
        type=pathlib.Path,
        help="ELF outside libdir whose DT_NEEDED dependencies must also be bundled",
    )
    args = parser.parse_args()
    count = bundle(args.libdir, args.search_dir, args.extra_elf)
    print(f"bundled {count} non-baseline runtime dependencies", file=sys.stderr)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
