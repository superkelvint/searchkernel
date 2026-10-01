#!/usr/bin/env python3
"""Stage only shared link inputs selected by the configured adapter command."""

from __future__ import annotations

import argparse
import pathlib
import shlex
import subprocess
import tempfile

from native_base_metadata import normalize_absolute_root
from native_runtime_closure import SYSTEM_BASELINE, elf_soname, is_elf, sha256_file


TEST_LIBRARIES = {
    "vespa_messagebus-test",
    "searchlib_test",
    "searchlib_searchlib_test_features",
    "searchlib_searchlib_test_memoryindex",
    "gtest",
}


def selected_shared_inputs(
    link_file: pathlib.Path,
    cxx: str,
    linker_aliases: dict[str, pathlib.Path],
    producer_roots: list[tuple[str, str]],
) -> list[pathlib.Path]:
    tokens = shlex.split(link_file.read_text(encoding="utf-8"))
    search_args: list[str] = []
    library_names: list[str] = []
    direct_paths: list[pathlib.Path] = []
    index = 0
    while index < len(tokens):
        token = tokens[index]
        if token == "-L" and index + 1 < len(tokens):
            search_path = tokens[index + 1]
            for source, destination in producer_roots:
                search_path = normalize_absolute_root(search_path, source, destination)
            search_args.extend(("-L", search_path))
            index += 2
            continue
        if token.startswith("-L"):
            search_path = token[2:]
            for source, destination in producer_roots:
                search_path = normalize_absolute_root(search_path, source, destination)
            search_args.append("-L" + search_path)
        elif token == "-l" and index + 1 < len(tokens):
            library_names.append(tokens[index + 1])
            index += 1
        elif token.startswith("-l"):
            library_names.append(token[2:])
        elif token.endswith((".so", ".so.")) or ".so." in token:
            direct_path = token
            for source, destination in producer_roots:
                direct_path = normalize_absolute_root(direct_path, source, destination)
            direct_paths.append(pathlib.Path(direct_path))
        index += 1

    selected: dict[pathlib.Path, str] = {}
    with tempfile.TemporaryDirectory(prefix="searchkernel-link-inputs.") as raw:
        probe = pathlib.Path(raw) / "probe.so"
        for name in library_names:
            if name.startswith(":") and name.endswith(".a"):
                continue
            if name.endswith(".a") or name in TEST_LIBRARIES:
                continue
            if name in linker_aliases:
                candidates = [linker_aliases[name]]
            else:
                result = subprocess.run(
                    [
                        cxx,
                        "-shared",
                        "-nostdlib",
                        "-Wl,-t",
                        "-Wl,--no-as-needed",
                        *search_args,
                        f"-l{name}",
                        "-o",
                        str(probe),
                    ],
                    cwd=link_file.parent.parent.parent,
                    check=False,
                    text=True,
                    stdout=subprocess.PIPE,
                    stderr=subprocess.PIPE,
                )
                if result.returncode != 0:
                    raise RuntimeError(
                        f"configured adapter link input -l{name} could not be resolved:\n"
                        f"{result.stderr.strip()}"
                    )
                candidates = [pathlib.Path(line.strip()) for line in result.stdout.splitlines()]
            for candidate in candidates:
                try:
                    resolved = candidate.resolve(strict=True)
                except (OSError, RuntimeError):
                    continue
                if not resolved.is_file() or not is_elf(resolved):
                    continue
                soname = elf_soname(resolved, "readelf")
                if soname in SYSTEM_BASELINE:
                    continue
                selected[resolved] = name

    for path in direct_paths:
        try:
            resolved = (path if path.is_absolute() else link_file.parent.parent.parent / path).resolve(strict=True)
        except (OSError, RuntimeError) as exc:
            raise RuntimeError(f"configured adapter link input is missing: {path}") from exc
        if resolved.is_file() and is_elf(resolved):
            soname = elf_soname(resolved, "readelf")
            if soname not in SYSTEM_BASELINE:
                selected[resolved] = path.name

    by_soname: dict[str, tuple[str, pathlib.Path]] = {}
    for path, link_name in selected.items():
        soname = elf_soname(path, "readelf")
        if not soname:
            raise RuntimeError(f"configured shared link input has no DT_SONAME: {path}")
        digest = sha256_file(path)
        previous = by_soname.get(soname)
        if previous and previous[0] != digest:
            raise RuntimeError(
                f"configured adapter link resolves conflicting binaries for {soname}: "
                f"{previous[1]} and {path} (link inputs {previous[1]} and {link_name})"
            )
        by_soname[soname] = (digest, path)
    if not by_soname:
        raise RuntimeError(f"configured adapter link command selected no producer shared libraries: {link_file}")
    return sorted((item[1] for item in by_soname.values()), key=str)


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("link_file", type=pathlib.Path)
    parser.add_argument("destination", type=pathlib.Path)
    parser.add_argument("--cxx", default="c++")
    parser.add_argument("--linker-alias", action="append", default=[], metavar="NAME=PATH")
    parser.add_argument("--producer-root", action="append", default=[], metavar="SOURCE=DESTINATION")
    args = parser.parse_args()
    aliases: dict[str, pathlib.Path] = {}
    for value in args.linker_alias:
        name, separator, raw_path = value.partition("=")
        if not separator or not name or not raw_path:
            raise SystemExit(f"invalid linker alias {value!r}; expected NAME=PATH")
        aliases[name] = pathlib.Path(raw_path)
    producer_roots: list[tuple[str, str]] = []
    for value in args.producer_root:
        source, separator, destination = value.partition("=")
        if not separator or not source or not destination:
            raise SystemExit(f"invalid producer root {value!r}; expected SOURCE=DESTINATION")
        producer_roots.append((source, destination))
    try:
        sources = selected_shared_inputs(
            args.link_file.resolve(strict=True), args.cxx, aliases, producer_roots
        )
    except (OSError, RuntimeError) as exc:
        raise SystemExit(str(exc)) from exc
    copy_helper = pathlib.Path(__file__).with_name("copy-shared-library-into-sdk.sh")
    subprocess.run(["sh", str(copy_helper), str(args.destination), *(str(path) for path in sources)], check=True)
    print(f"staged {len(sources)} adapter link-selected runtime DSOs")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
