#!/usr/bin/env python3
"""Load a packaged Search Kernel adapter and call its stable ABI probe."""

from __future__ import annotations

import argparse
import os
import pathlib
import shutil
import subprocess
import sys
import tempfile


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("adapter", type=pathlib.Path)
    parser.add_argument("expected_abi", type=int)
    args = parser.parse_args()
    adapter = args.adapter.resolve(strict=True)
    compiler = shutil.which("cc")
    if compiler is None:
        raise SystemExit("cc is required for native adapter runtime smoke")
    with tempfile.TemporaryDirectory(prefix="searchkernel-native-smoke.") as raw:
        work = pathlib.Path(raw)
        source = work / "smoke.c"
        executable = work / "smoke"
        source.write_text(
            "#include <stdint.h>\n"
            "extern uint32_t sk_native_abi_version(void);\n"
            f"int main(void) {{ return sk_native_abi_version() == {args.expected_abi}U ? 0 : 1; }}\n",
            encoding="utf-8",
        )
        result = subprocess.run(
            [
                compiler,
                str(source),
                str(adapter),
                f"-Wl,-rpath,{adapter.parent}",
                "-o",
                str(executable),
            ],
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )
        if result.returncode != 0:
            raise SystemExit(f"could not link native adapter runtime smoke:\n{result.stderr.strip()}")
        env = os.environ.copy()
        env.pop("LD_LIBRARY_PATH", None)
        env.pop("LD_PRELOAD", None)
        run = subprocess.run(
            [str(executable)],
            env=env,
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )
        if run.returncode != 0:
            detail = run.stderr.strip() or f"smoke process exited with status {run.returncode}"
            raise SystemExit(f"dynamic loader/runtime could not run packaged Search Kernel adapter: {detail}")
    print(f"native adapter runtime load and ABI smoke: PASS ({args.expected_abi})")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
