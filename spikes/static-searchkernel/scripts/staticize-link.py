#!/usr/bin/env python3
"""Build a genuinely more-static Search Kernel native link plan for the spike."""

from __future__ import annotations

import argparse
import json
import os
import re
import resource
import shlex
import subprocess
import sys
import time
from pathlib import Path

SYSTEM_LIBS = {
    "c", "m", "dl", "pthread", "rt", "stdc++", "gcc", "gcc_s",
    "atomic", "resolv", "util",
}
TEST_MARKERS = (
    "libsearchcore_test.a",
    "libvespa_messagebus-test.so",
    "libsearchlib_test.so",
    "libsearchlib_searchlib_test_features.so",
    "libsearchlib_searchlib_test_memoryindex.so",
    "libgtest.so",
)


def parse_args() -> argparse.Namespace:
    p = argparse.ArgumentParser()
    p.add_argument("--vespa-root", required=True, type=Path)
    p.add_argument("--sdk", required=True, type=Path)
    p.add_argument("--object", required=True, type=Path)
    p.add_argument("--shared-output", required=True, type=Path)
    p.add_argument("--version-script", required=True, type=Path)
    p.add_argument("--results", required=True, type=Path)
    p.add_argument("--cxx", default=os.environ.get("CXX", "c++"))
    return p.parse_args()


def configured_files(vespa: Path) -> tuple[Path, Path]:
    base = vespa / "searchcore/src/tests/proton/documentdb/CMakeFiles/searchcore_embedded_crud_spike_app.dir"
    return base / "link.txt", base / "flags.make"


def configured_deps(flags: Path) -> Path | None:
    text = flags.read_text(encoding="utf-8")
    line = next((x for x in text.splitlines() if x.startswith("CXX_INCLUDES = ")), "")
    tokens = shlex.split(line.partition("=")[2])
    for i, token in enumerate(tokens):
        value: str | None = None
        if token == "-isystem" and i + 1 < len(tokens):
            value = tokens[i + 1]
        elif token.startswith("-isystem") and token != "-isystem":
            value = token[len("-isystem"):]
        if value and value.endswith("/include"):
            return Path(value).parent
    return None


def choose_deps_root(vespa: Path, configured: Path | None) -> Path | None:
    env = os.environ.get("VESPA_DEPS")
    candidates = [
        Path(env) if env else None,
        vespa.parent.parent / "vespa-deps" if vespa.name == "vespa" else None,
        configured,
    ]
    for candidate in candidates:
        if candidate and candidate.is_dir():
            return candidate.resolve()
    return configured.resolve() if configured and configured.exists() else None


def remap(token: str, vespa: Path, configured: Path | None, deps: Path | None) -> str:
    value = token.replace("/work/vespa", str(vespa))
    if configured and deps:
        value = value.replace(str(configured), str(deps))
    return value


def strip_to_compiler_args(tokens: list[str]) -> tuple[list[str], str]:
    """Drop CMake/shell wrapper tokens and return arguments after the real C++ driver."""
    compiler_names = {"c++", "g++", "clang++"}
    for i, token in enumerate(tokens):
        if Path(token).name in compiler_names:
            return tokens[i + 1 :], token
    raise SystemExit(
        "configured link command did not contain a recognizable C++ driver: "
        + shlex.join(tokens)
    )


def absolutize_link_input(token: str, cwd: Path) -> str:
    """Resolve file/search-path arguments exactly as CMake's original link cwd would."""
    if token.startswith("-L") and len(token) > 2:
        path = Path(token[2:])
        if not path.is_absolute():
            path = (cwd / path).resolve()
        return "-L" + str(path)

    if token.startswith("-"):
        return token

    # Linker file inputs in Vespa's generated command are often relative to the
    # target build directory. Preserve non-path words, but canonicalize anything
    # that looks like an object/archive/shared-library/path.
    looks_like_path = (
        "/" in token
        or token.endswith(".o")
        or token.endswith(".a")
        or ".so" in Path(token).name
    )
    if not looks_like_path:
        return token

    path = Path(token)
    if not path.is_absolute():
        path = (cwd / path).resolve()
    return str(path)


def build_archive_index(roots: list[Path]) -> dict[str, list[Path]]:
    index: dict[str, list[Path]] = {}
    seen: set[Path] = set()
    for root in roots:
        if not root or not root.exists():
            continue
        for path in root.rglob("*.a"):
            try:
                resolved = path.resolve()
            except OSError:
                continue
            if resolved in seen:
                continue
            seen.add(resolved)
            index.setdefault(path.name, []).append(resolved)
    return index


def static_name(token: str) -> tuple[str, str] | None:
    if token.startswith("-l:"):
        name = token[3:]
        match = re.match(r"^(lib.+?)\.so(?:\..*)?$", name)
        return (match.group(1) + ".a", match.group(1)[3:]) if match else None
    if token.startswith("-l") and len(token) > 2:
        logical = token[2:]
        if logical in SYSTEM_LIBS:
            return None
        return f"lib{logical}.a", logical
    base = Path(token).name
    match = re.match(r"^(lib.+?)\.so(?:\..*)?$", base)
    if match:
        return match.group(1) + ".a", match.group(1)[3:]
    return None


def non_test(paths: list[Path]) -> list[Path]:
    result = []
    for path in paths:
        lowered = str(path).lower()
        if "/test/" in lowered or "/tests/" in lowered or "-test" in path.name:
            continue
        result.append(path)
    return result


def choose_archive(token: str, index: dict[str, list[Path]]) -> tuple[Path | None, str]:
    info = static_name(token)
    if not info:
        return None, "not-candidate"
    archive_name, _ = info
    candidates = non_test(index.get(archive_name, []))
    if Path(token).is_absolute():
        sibling = Path(token).parent / archive_name
        if sibling.is_file():
            return sibling.resolve(), "sibling"
    if len(candidates) == 1:
        return candidates[0], "unique"
    if not candidates:
        return None, "missing"
    return None, "ambiguous:" + ",".join(str(x) for x in candidates)


def strip_output_and_test_inputs(args: list[str], replacement_object: Path) -> list[str]:
    out: list[str] = []
    skip = False
    replaced_object = False
    for token in args:
        if skip:
            skip = False
            continue
        if token == "-o":
            skip = True
            continue
        if any(marker in token for marker in TEST_MARKERS):
            continue
        if token.endswith("embedded_crud_spike.cpp.o"):
            out.append(str(replacement_object))
            replaced_object = True
            continue
        out.append(token)
    if not replaced_object:
        raise SystemExit("configured link line did not contain embedded_crud_spike.cpp.o")
    return out


def remove_link_mode_flags(args: list[str]) -> list[str]:
    result = []
    for token in args:
        if token == "-shared" or token == "-pie":
            continue
        if token == "-rdynamic":
            # The production adapter strips this; final Rust links do not need it either.
            continue
        if token.startswith("-Wl,-soname,"):
            continue
        if token.startswith("-Wl,-rpath,") or token.startswith("-Wl,--disable-new-dtags,-rpath,"):
            continue
        result.append(token)
    return result


def staticize(args: list[str], index: dict[str, list[Path]], results: Path) -> list[str]:
    rewritten: list[str] = []
    replacements: list[tuple[str, str, str]] = []
    unresolved: list[tuple[str, str]] = []

    for token in args:
        if token.endswith(".a"):
            rewritten.append(token)
            continue
        archive, reason = choose_archive(token, index)
        if archive:
            rewritten.append(str(archive))
            replacements.append((token, str(archive), reason))
        else:
            rewritten.append(token)
            if static_name(token):
                unresolved.append((token, reason))

    results.mkdir(parents=True, exist_ok=True)
    with (results / "static-replacements.tsv").open("w", encoding="utf-8") as stream:
        stream.write("dynamic_input\tstatic_archive\treason\n")
        for old, new, reason in replacements:
            stream.write(f"{old}\t{new}\t{reason}\n")
    with (results / "unresolved-static-candidates.tsv").open("w", encoding="utf-8") as stream:
        stream.write("dynamic_input\treason\n")
        for token, reason in unresolved:
            stream.write(f"{token}\t{reason}\n")
    return rewritten


def main() -> int:
    ns = parse_args()
    vespa = ns.vespa_root.resolve()
    sdk = ns.sdk.resolve()
    obj = ns.object.resolve()
    link_file, flags_file = configured_files(vespa)
    if not link_file.is_file() or not flags_file.is_file():
        raise SystemExit("configured embedded CRUD link metadata is missing")
    if not obj.is_file():
        raise SystemExit(f"adapter object is missing: {obj}")

    configured = configured_deps(flags_file)
    deps = choose_deps_root(vespa, configured)

    raw = link_file.read_text(encoding="utf-8").strip()
    tokens = shlex.split(raw)
    if not tokens:
        raise SystemExit("empty configured link command")

    # CMake may prefix the compiler with shell/env wrapper tokens. Locate the
    # actual C++ driver instead of assuming tokens[0]. The generated relative
    # object/archive paths are interpreted from the target's CMake directory,
    # exactly as core/native/vespa/build-sdk.sh does before eval'ing the link.
    tokens, configured_cxx = strip_to_compiler_args(tokens)
    link_cwd = link_file.parent.parent.parent
    tokens = [remap(x, vespa, configured, deps) for x in tokens]
    tokens = [absolutize_link_input(x, link_cwd) for x in tokens]
    tokens = strip_output_and_test_inputs(tokens, obj)
    tokens = remove_link_mode_flags(tokens)

    roots = [vespa]
    if deps:
        roots.append(deps)
    archive_index = build_archive_index(roots)
    rewritten = staticize(tokens, archive_index, ns.results)

    # Search the immutable SDK first for any dynamic inputs that remain. Static
    # archives are explicit absolute paths, so this does not change what was
    # absorbed into the candidate.
    common = [f"-L{sdk / 'lib'}", *rewritten]

    shared_args = [
        *common,
        "-shared",
        "-Wl,-soname,libsearchkernel_vespa.so.2",
        "-Wl,--disable-new-dtags,-rpath,$ORIGIN",
        "-Wl,--exclude-libs,ALL",
        f"-Wl,--version-script={ns.version_script.resolve()}",
        "-o",
        str(ns.shared_output.resolve()),
    ]
    executable_extra = [
        *common,
        # Candidate A is about absorbing Search Kernel/Vespa native code, not
        # preserving PIE. Some usable static archives may not be PIC even though
        # they are valid for a normal executable.
        "-no-pie",
        "-Wl,--disable-new-dtags,-rpath,$ORIGIN/.searchkernel-native",
    ]

    ns.results.mkdir(parents=True, exist_ok=True)
    (ns.results / "plan.json").write_text(
        json.dumps(
            {
                "configured_link_file": str(link_file),
                "configured_link_cwd": str(link_cwd),
                "configured_cxx": configured_cxx,
                "configured_deps": str(configured) if configured else None,
                "deps_root": str(deps) if deps else None,
                "shared_args": shared_args,
                "executable_extra_args": executable_extra,
            },
            indent=2,
        )
        + "\n",
        encoding="utf-8",
    )
    (ns.results / "executable-extra-args.json").write_text(
        json.dumps(executable_extra, indent=2) + "\n",
        encoding="utf-8",
    )
    (ns.results / "shared-command.txt").write_text(
        shlex.join([ns.cxx, *shared_args]) + "\n", encoding="utf-8"
    )

    ns.shared_output.parent.mkdir(parents=True, exist_ok=True)
    started = time.monotonic()
    before = resource.getrusage(resource.RUSAGE_CHILDREN).ru_maxrss
    with (ns.results / "candidate-b-link.log").open("w", encoding="utf-8") as log:
        proc = subprocess.run(
            [ns.cxx, *shared_args],
            stdout=log,
            stderr=subprocess.STDOUT,
            check=False,
        )
    elapsed = time.monotonic() - started
    after = resource.getrusage(resource.RUSAGE_CHILDREN).ru_maxrss
    (ns.results / "candidate-b-link-metrics.json").write_text(
        json.dumps(
            {
                "exit_code": proc.returncode,
                "wall_seconds": elapsed,
                "peak_child_rss_kib": max(before, after),
            },
            indent=2,
        )
        + "\n",
        encoding="utf-8",
    )
    return proc.returncode


if __name__ == "__main__":
    sys.exit(main())
