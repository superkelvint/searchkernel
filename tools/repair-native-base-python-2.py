#!/usr/bin/env python3
import pathlib, shlex, shutil, sys
link=pathlib.Path(sys.argv[1]).resolve(); vr=pathlib.Path(sys.argv[2]).resolve(); dr=pathlib.Path(sys.argv[3]).resolve(); pv=pathlib.Path(sys.argv[4]); pd=pathlib.Path(sys.argv[5]); cwd=link.parent.parent.parent
for token in shlex.split(link.read_text(encoding="utf-8")):
    if not token.endswith(".a") or pathlib.Path(token).name == "libsearchcore_test.a": continue
    src=pathlib.Path(token)
    if not src.is_absolute(): src=cwd/src
    src=src.resolve(strict=True)
    try: dst=pv/src.relative_to(vr)
    except ValueError:
        try: dst=pd/src.relative_to(dr)
        except ValueError as exc: raise SystemExit(f"static archive outside declared roots: {src}") from exc
    dst.parent.mkdir(parents=True, exist_ok=True); shutil.copy2(src,dst)
