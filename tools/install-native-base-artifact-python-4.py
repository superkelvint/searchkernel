#!/usr/bin/env python3
import pathlib
import shlex
import sys

link_file = pathlib.Path(sys.argv[1]).resolve()
vespa_root = pathlib.Path(sys.argv[2]).resolve()

for token in shlex.split(link_file.read_text(encoding="utf-8")):
    if not token.endswith(".a"):
        continue
    if pathlib.Path(token).name == "libsearchcore_test.a":
        continue

    if token.startswith("/work/vespa/"):
        archive = vespa_root / token[len("/work/vespa/"):]
    else:
        archive = pathlib.Path(token)
        if not archive.is_absolute():
            # CMake evaluates link.txt arguments from the target directory,
            # one level above CMakeFiles/<target>.dir.
            archive = link_file.parent.parent.parent / archive

    archive = archive.resolve()
    try:
        archive.relative_to(vespa_root)
    except ValueError as exc:
        raise SystemExit(
            f"published Vespa link references static archive outside packaged toolchain: {token}"
        ) from exc

    if not archive.is_file():
        raise SystemExit(f"published Vespa base is missing adapter static archive: {token}")
