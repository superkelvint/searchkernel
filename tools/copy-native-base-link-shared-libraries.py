#!/usr/bin/env python3
"""Stage only shared link inputs selected by the configured adapter command."""

from __future__ import annotations

import argparse
import json
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
) -> dict[pathlib.Path, set[str]]:
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

    selected: dict[pathlib.Path, set[str]] = {}
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
                if elf_soname(resolved, "readelf") in SYSTEM_BASELINE:
                    continue
                # Preserve the name used by the configured linker command.
                # Resolving the input symlink gives us the exact ELF payload,
                # but loses e.g. libfoo.so when the payload is libfoo.so.3.4.
                selected.setdefault(resolved, set()).add(f"lib{name}.so")

    for path in direct_paths:
        try:
            resolved = (path if path.is_absolute() else link_file.parent.parent.parent / path).resolve(strict=True)
        except (OSError, RuntimeError) as exc:
            raise RuntimeError(f"configured adapter link input is missing: {path}") from exc
        if resolved.is_file() and is_elf(resolved):
            if elf_soname(resolved, "readelf") not in SYSTEM_BASELINE:
                selected.setdefault(resolved, set()).add(path.name)

    by_soname: dict[str, tuple[str, pathlib.Path]] = {}
    aliases_by_path: dict[pathlib.Path, set[str]] = {}
    for path, aliases in selected.items():
        soname = elf_soname(path, "readelf")
        if not soname:
            raise RuntimeError(f"configured shared link input has no DT_SONAME: {path}")
        digest = sha256_file(path)
        previous = by_soname.get(soname)
        if previous and previous[0] != digest:
            raise RuntimeError(
                f"configured adapter link resolves conflicting binaries for {soname}: "
                f"{previous[1]} and {path}"
            )
        by_soname[soname] = (digest, path)
        aliases_by_path.setdefault(path, set()).update(aliases)
    if not by_soname:
        raise RuntimeError(f"configured adapter link command selected no producer shared libraries: {link_file}")
    result: dict[pathlib.Path, set[str]] = {}
    for _, path in by_soname.values():
        result[path] = set(aliases_by_path[path])
    return result


def source_identity(path: pathlib.Path, roots: list[tuple[str, pathlib.Path]]) -> str:
    resolved = path.resolve(strict=True)
    for label, root in roots:
        try:
            return f"{label}/{resolved.relative_to(root)}"
        except ValueError:
            pass
    raise RuntimeError(f"configured adapter link input escapes declared producer roots: {resolved}")


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("link_file", type=pathlib.Path)
    parser.add_argument("destination", type=pathlib.Path)
    parser.add_argument("--cxx", default="c++")
    parser.add_argument("--linker-alias", action="append", default=[], metavar="NAME=PATH")
    parser.add_argument("--producer-root", action="append", default=[], metavar="SOURCE=DESTINATION")
    parser.add_argument("--source-root", action="append", default=[], metavar="LABEL=PATH")
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
    source_roots: list[tuple[str, pathlib.Path]] = []
    for value in args.source_root:
        label, separator, raw_path = value.partition("=")
        if not separator or not label or not raw_path:
            raise SystemExit(f"invalid source root {value!r}; expected LABEL=PATH")
        source_roots.append((label, pathlib.Path(raw_path).resolve(strict=True)))
    try:
        aliases_by_path = selected_shared_inputs(
            args.link_file.resolve(strict=True), args.cxx, aliases, producer_roots
        )
        sources = sorted(aliases_by_path, key=str)
        copy_helper = pathlib.Path(__file__).with_name("copy-shared-library-into-sdk.sh")
        subprocess.run(["sh", str(copy_helper), str(args.destination), *(str(path) for path in sources)], check=True)
        # The adapter is rebuilt from this package and needs the same linker
        # names selected from link.txt. Keep each as a package-local alias to
        # the exact ELF already staged under its SONAME.
        for source in sources:
            soname = elf_soname(source, "readelf")
            for alias in sorted(aliases_by_path.get(source, ())):
                if pathlib.Path(alias).name != alias or alias in {"", ".", ".."}:
                    raise RuntimeError(f"unsafe configured linker alias: {alias!r}")
                alias_path = args.destination / alias
                target_path = args.destination / soname
                if alias == soname:
                    continue
                if alias_path.exists() or alias_path.is_symlink():
                    try:
                        if alias_path.resolve(strict=True) == target_path.resolve(strict=True):
                            continue
                    except (OSError, RuntimeError):
                        pass
                    raise RuntimeError(f"conflicting staged linker alias {alias_path}")
                alias_path.symlink_to(soname)
        source_by_hash = {
            sha256_file(path): source_identity(path, source_roots)
            for path in sources
        }
        readelf = "readelf"
        entries = []
        seen: set[pathlib.Path] = set()
        destination = args.destination.resolve(strict=True)
        for entry in sorted(destination.iterdir(), key=lambda item: item.name):
            payload = entry.resolve(strict=True)
            if payload in seen or not payload.is_file() or not is_elf(payload):
                continue
            seen.add(payload)
            digest = sha256_file(payload)
            entries.append({
                "packaged_name": entry.name,
                "soname": elf_soname(payload, readelf),
                "sha256": digest,
                "source_identity": source_by_hash[digest],
                "required_by": ["configured-adapter-link"],
            })
        provenance = destination / ".searchkernel-runtime-provenance.json"
        provenance.write_text(
            json.dumps({"format_version": 1, "libraries": entries}, indent=2, sort_keys=True) + "\n",
            encoding="utf-8",
        )
    except (OSError, RuntimeError) as exc:
        raise SystemExit(str(exc)) from exc
    print(f"staged {len(sources)} adapter link-selected runtime DSOs")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
