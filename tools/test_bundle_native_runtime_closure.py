#!/usr/bin/env python3
"""Regression coverage for producer-side native runtime dependency bundling."""

from __future__ import annotations

import os
import pathlib
import shutil
import subprocess
import sys
import tempfile


ROOT = pathlib.Path(__file__).resolve().parents[1]
BUNDLER = ROOT / "tools" / "bundle-native-runtime-closure.py"
CHECKER = ROOT / "tools" / "check-native-runtime-closure.py"


def run(*args: str, env: dict[str, str] | None = None) -> None:
    subprocess.run(args, check=True, env=env)


def compile_shared(
    cc: str,
    source: pathlib.Path,
    output: pathlib.Path,
    soname: str,
    *extra: str,
) -> None:
    run(
        cc,
        "-shared",
        "-fPIC",
        f"-Wl,-soname,{soname}",
        *extra,
        "-o",
        str(output),
        str(source),
    )


def main() -> int:
    cc = shutil.which("cc")
    if cc is None:
        raise SystemExit("cc is required for native runtime bundling regression")

    with tempfile.TemporaryDirectory(prefix="searchkernel-runtime-bundle.") as raw:
        root = pathlib.Path(raw)
        producer = root / "producer"
        bundle = root / "bundle"
        producer.mkdir()
        bundle.mkdir()

        leaf_name = "libsearchkernel_fixture_leaf.so.1"
        mid_name = "libsearchkernel_fixture_mid.so.1"
        consumer_name = "libvespa_searchkernel_fixture.so.1"

        (root / "leaf.c").write_text(
            "int searchkernel_fixture_leaf(void) { return 7; }\n",
            encoding="utf-8",
        )
        (root / "mid.c").write_text(
            "extern int searchkernel_fixture_leaf(void);\n"
            "int searchkernel_fixture_mid(void) { return searchkernel_fixture_leaf(); }\n",
            encoding="utf-8",
        )
        (root / "consumer.c").write_text(
            "extern int searchkernel_fixture_mid(void);\n"
            "int searchkernel_fixture_consumer(void) { return searchkernel_fixture_mid(); }\n",
            encoding="utf-8",
        )

        leaf = producer / leaf_name
        compile_shared(cc, root / "leaf.c", leaf, leaf_name)
        (producer / "libsearchkernel_fixture_leaf.so").symlink_to(leaf.name)

        mid = producer / mid_name
        compile_shared(
            cc,
            root / "mid.c",
            mid,
            mid_name,
            "-L",
            str(producer),
            "-Wl,--no-as-needed",
            "-lsearchkernel_fixture_leaf",
        )
        (producer / "libsearchkernel_fixture_mid.so").symlink_to(mid.name)

        consumer = bundle / consumer_name
        compile_shared(
            cc,
            root / "consumer.c",
            consumer,
            consumer_name,
            "-L",
            str(producer),
            "-Wl,--no-as-needed",
            "-lsearchkernel_fixture_mid",
        )

        before = subprocess.run(
            [sys.executable, str(CHECKER), str(bundle)],
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )
        if before.returncode == 0:
            raise SystemExit("closure checker unexpectedly accepted unbundled fixture dependencies")
        if mid_name not in before.stderr:
            raise SystemExit(
                "pre-bundle closure check failed for the wrong reason:\n" + before.stderr
            )

        env = os.environ.copy()
        env.pop("LD_PRELOAD", None)
        run(
            sys.executable,
            str(BUNDLER),
            str(bundle),
            "--search-dir",
            str(producer),
            env=env,
        )

        for name in (mid_name, leaf_name):
            path = bundle / name
            if not path.is_file():
                raise SystemExit(f"bundler did not copy recursive runtime dependency {name}")
            if path.read_bytes()[:4] != b"\\x7fELF":
                raise SystemExit(f"bundled runtime dependency is not ELF: {name}")

        complete = subprocess.run(
            [sys.executable, str(CHECKER), str(bundle)],
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )
        if complete.returncode != 0:
            raise SystemExit(
                "closure checker rejected recursively bundled runtime closure:\n"
                + complete.stderr
            )

    print("native runtime bundling regression: PASS")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
