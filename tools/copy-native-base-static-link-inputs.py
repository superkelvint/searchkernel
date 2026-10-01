#!/usr/bin/env python3
"""Copy static archives referenced by a configured Vespa link command."""

from __future__ import annotations

import argparse
import pathlib
import shlex
import shutil

from native_base_metadata import normalize_absolute_root


def copy_static_inputs(
    link_file: pathlib.Path,
    vespa_root: pathlib.Path,
    deps_root: pathlib.Path,
    packaged_vespa: pathlib.Path,
    packaged_deps: pathlib.Path,
    producer_roots: list[tuple[str, str]],
) -> int:
    link_file = link_file.resolve(strict=True)
    vespa_root = vespa_root.resolve(strict=True)
    deps_root = deps_root.resolve(strict=True)
    link_cwd = link_file.parent.parent.parent
    copied = 0

    for token in shlex.split(link_file.read_text(encoding="utf-8")):
        if not token.endswith(".a") or pathlib.Path(token).name == "libsearchcore_test.a":
            continue
        for source_root, destination_root in producer_roots:
            token = normalize_absolute_root(token, source_root, destination_root)
        source = pathlib.Path(token)
        if not source.is_absolute():
            # CMake evaluates paths in link.txt from the target binary directory.
            source = link_cwd / source
        source = source.resolve(strict=True)
        if not source.is_file():
            raise RuntimeError(f"adapter link static archive is missing: {source}")
        try:
            relative = source.relative_to(vespa_root)
            destination_root = packaged_vespa
        except ValueError:
            try:
                relative = source.relative_to(deps_root)
                destination_root = packaged_deps
            except ValueError as exc:
                raise RuntimeError(
                    f"adapter link references static archive outside declared producer roots: {token}"
                ) from exc
        destination = destination_root / relative
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(source, destination)
        copied += 1
    return copied


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("link_file", type=pathlib.Path)
    parser.add_argument("vespa_root", type=pathlib.Path)
    parser.add_argument("deps_root", type=pathlib.Path)
    parser.add_argument("packaged_vespa", type=pathlib.Path)
    parser.add_argument("packaged_deps", type=pathlib.Path)
    parser.add_argument("--producer-root", action="append", default=[], metavar="SOURCE=DESTINATION")
    args = parser.parse_args()
    producer_roots: list[tuple[str, str]] = []
    for value in args.producer_root:
        source, separator, destination = value.partition("=")
        if not separator or not source or not destination:
            raise SystemExit(f"invalid producer root {value!r}; expected SOURCE=DESTINATION")
        producer_roots.append((source, destination))
    try:
        copied = copy_static_inputs(
            args.link_file,
            args.vespa_root,
            args.deps_root,
            args.packaged_vespa,
            args.packaged_deps,
            producer_roots,
        )
    except (OSError, RuntimeError) as exc:
        raise SystemExit(str(exc)) from exc
    print(f"packaged {copied} Vespa static link archives")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
