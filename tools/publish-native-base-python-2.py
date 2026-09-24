#!/usr/bin/env python3
import pathlib, shlex, shutil, sys
link_file = pathlib.Path(sys.argv[1]).resolve()
vespa_root = pathlib.Path(sys.argv[2]).resolve()
deps_root = pathlib.Path(sys.argv[3]).resolve()
packaged_vespa = pathlib.Path(sys.argv[4])
packaged_deps = pathlib.Path(sys.argv[5])
link_cwd = link_file.parent.parent.parent
seen = set()
count = 0
for token in shlex.split(link_file.read_text(encoding="utf-8")):
    if not token.endswith(".a") or pathlib.Path(token).name == "libsearchcore_test.a":
        continue
    source = pathlib.Path(token)
    if not source.is_absolute():
        source = link_cwd / source
    source = source.resolve(strict=True)
    if str(source) in seen:
        continue
    seen.add(str(source))
    try:
        rel = source.relative_to(vespa_root)
        dest = packaged_vespa / rel
    except ValueError:
        try:
            rel = source.relative_to(deps_root)
            dest = packaged_deps / rel
        except ValueError as exc:
            raise SystemExit(f"adapter static archive is outside declared roots: {source}") from exc
    dest.parent.mkdir(parents=True, exist_ok=True)
    shutil.copy2(source, dest)
    count += 1
if count == 0:
    raise SystemExit("canonical adapter link command contained no production static archives")
print(f"packaged {count} production static archives")
