#!/usr/bin/env python3
"""Shared ELF runtime-closure policy for Search Kernel native-base tooling."""

from __future__ import annotations

import pathlib
import re
import subprocess
import hashlib
import os
import tempfile


# GNU/Linux C/C++ runtime baseline for the supported x86_64-unknown-linux-gnu
# native target. Product/dependency DSOs are deliberately absent: if Vespa
# needs one of those, the native-base artifact must carry it.
SYSTEM_BASELINE = {
    "ld-linux-x86-64.so.2",
    "libc.so.6",
    "libdl.so.2",
    "libgcc_s.so.1",
    "libm.so.6",
    "libpthread.so.0",
    "libresolv.so.2",
    "librt.so.1",
    "libstdc++.so.6",
    "libutil.so.1",
}

NEEDED_RE = re.compile(r"\(NEEDED\).*Shared library: \[(?P<name>[^\]]+)\]")
SONAME_RE = re.compile(r"\(SONAME\).*Library soname: \[(?P<name>[^\]]+)\]")
LDD_RESOLUTION_RE = re.compile(
    r"^\s*(?P<name>\S+)\s+=>\s+(?P<path>/\S+)\s+\(0x[0-9a-fA-F]+\)\s*$"
)
LDD_DIRECT_RE = re.compile(r"^\s*(?P<path>/\S+)\s+\(0x[0-9a-fA-F]+\)\s*$")


def is_elf(path: pathlib.Path) -> bool:
    try:
        with path.open("rb") as stream:
            return stream.read(4) == b"\x7fELF"
    except OSError:
        return False


def resolve_origin_dependency(payload: pathlib.Path, needed: str) -> pathlib.Path | None:
    """Resolve an ELF DT_NEEDED entry expressed relative to its executable origin."""
    for prefix in ("$ORIGIN/", "${ORIGIN}/"):
        if needed.startswith(prefix):
            relative = needed[len(prefix):]
            candidate = payload.parent / relative
            try:
                resolved = candidate.resolve(strict=True)
            except OSError:
                return None
            if resolved.is_file() and is_elf(resolved):
                return resolved
            return None
    return None


def needed_libraries(path: pathlib.Path, readelf: str) -> set[str]:
    completed = subprocess.run(
        [readelf, "-d", str(path)],
        check=False,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    if completed.returncode != 0:
        raise RuntimeError(
            "could not inspect native runtime dependencies for {}:\n{}".format(
                path, completed.stderr.strip()
            )
        )
    return {
        match.group("name")
        for line in completed.stdout.splitlines()
        if (match := NEEDED_RE.search(line)) is not None
    }


def elf_soname(path: pathlib.Path, readelf: str) -> str | None:
    completed = subprocess.run(
        [readelf, "-d", str(path)],
        check=False,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    if completed.returncode != 0:
        raise RuntimeError(f"could not inspect ELF SONAME for {path}: {completed.stderr.strip()}")
    return next(
        (match.group("name") for line in completed.stdout.splitlines() if (match := SONAME_RE.search(line))),
        None,
    )


def sha256_file(path: pathlib.Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def runtime_candidates(
    name: str,
    directories: list[pathlib.Path],
    allowed_roots: list[pathlib.Path],
) -> list[pathlib.Path]:
    """Find concrete same-name ELF candidates without following out-of-root aliases."""
    candidates: set[pathlib.Path] = set()
    for directory in directories:
        entry = directory / name
        try:
            resolved = entry.resolve(strict=True)
        except (OSError, RuntimeError):
            continue
        if _inside(resolved, allowed_roots) and resolved.is_file() and is_elf(resolved):
            candidates.add(resolved)
    return sorted(candidates, key=str)


def _inside(path: pathlib.Path, roots: list[pathlib.Path]) -> bool:
    return any(path.is_relative_to(root) for root in roots)


def _hermetic_search_dirs(
    library_path: list[pathlib.Path],
    allowed_roots: list[pathlib.Path],
    temporary_root: pathlib.Path,
) -> list[pathlib.Path]:
    """Mirror only ELF candidates whose resolved bytes live under allowed producer roots."""
    safe_dirs: list[pathlib.Path] = []
    for index, directory in enumerate(library_path):
        safe_dir = temporary_root / str(index)
        safe_dir.mkdir()
        for entry in sorted(directory.iterdir(), key=lambda item: item.name):
            try:
                resolved = entry.resolve(strict=True)
            except (OSError, RuntimeError):
                continue
            if not _inside(resolved, allowed_roots):
                continue
            if not resolved.is_file() or not is_elf(resolved):
                continue
            (safe_dir / entry.name).symlink_to(resolved)
        if any(safe_dir.iterdir()):
            safe_dirs.append(safe_dir)
    return safe_dirs


def ldd_resolutions(
    payload: pathlib.Path,
    ldd: str,
    library_path: list[pathlib.Path],
    allowed_roots: list[pathlib.Path] | None = None,
) -> dict[str, pathlib.Path]:
    """Return concrete loader choices without honoring stale external search aliases."""
    roots = [root.resolve(strict=True) for root in (allowed_roots or library_path)]
    env = os.environ.copy()
    for variable in tuple(env):
        if variable.startswith("LD_"):
            env.pop(variable, None)

    with tempfile.TemporaryDirectory(prefix="searchkernel-ldd-path.") as raw:
        sanitized = _hermetic_search_dirs(library_path, roots, pathlib.Path(raw))
        if sanitized:
            env["LD_LIBRARY_PATH"] = ":".join(str(path) for path in sanitized)
        completed = subprocess.run(
            [ldd, "-r", str(payload)],
            check=False,
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            env=env,
        )
        output = completed.stdout + "\n" + completed.stderr
        failures = [
            line.strip()
            for line in output.splitlines()
            if "not found" in line or "undefined symbol" in line
        ]
        if failures:
            raise RuntimeError("loader rejected {}:\n  {}".format(payload, "\n  ".join(failures)))

        resolved: dict[str, pathlib.Path] = {}
        for line in completed.stdout.splitlines():
            match = LDD_RESOLUTION_RE.match(line)
            if match:
                resolved[match.group("name")] = pathlib.Path(match.group("path")).resolve(strict=True)
                continue
            match = LDD_DIRECT_RE.match(line)
            if match:
                path = pathlib.Path(match.group("path")).resolve(strict=True)
                resolved[path.name] = path
        return resolved
