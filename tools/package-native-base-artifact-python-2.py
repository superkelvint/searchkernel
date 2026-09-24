#!/usr/bin/env python3
import pathlib
import shlex
import shutil
import sys

link_file = pathlib.Path(sys.argv[1]).resolve()
vespa_root = pathlib.Path(sys.argv[2]).resolve()
destination_root = pathlib.Path(sys.argv[3])

tokens = shlex.split(link_file.read_text(encoding="utf-8"))
for token in tokens:
    if not token.endswith(".a"):
        continue
    if pathlib.Path(token).name == "libsearchcore_test.a":
        continue

    source = pathlib.Path(token)
    if not source.is_absolute():
        # CMake writes link.txt below CMakeFiles/<target>.dir, but evaluates
        # relative link arguments from the target's binary directory.
        source = link_file.parent.parent.parent / source
    source = source.resolve()

    try:
        relative = source.relative_to(vespa_root)
    except ValueError as exc:
        raise SystemExit(
            f"adapter link references static archive outside Vespa root: {token}"
        ) from exc

    if not source.is_file():
        raise SystemExit(f"adapter link static archive is missing: {source}")

    destination = destination_root / relative
    destination.parent.mkdir(parents=True, exist_ok=True)
    shutil.copy2(source, destination)
