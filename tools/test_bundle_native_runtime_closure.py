#!/usr/bin/env python3
"""Regression coverage for producer-side native runtime dependency bundling."""

from __future__ import annotations

import os
import pathlib
import shutil
import stat
import subprocess
import sys
import tempfile


ROOT = pathlib.Path(__file__).resolve().parents[1]
BUNDLER = ROOT / "tools" / "bundle-native-runtime-closure.py"
CHECKER = ROOT / "tools" / "check-native-runtime-closure.py"
PACKAGE = ROOT / "tools" / "package-native-base-artifact.sh"
MAKE_WRITABLE_TREE = ROOT / "tools" / "make-writable-tree.sh"


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

        package_text = PACKAGE.read_text(encoding="utf-8")
        copied_base = package_text.index('cp -aL "$BASE/." "$BUNDLE/base/"')
        prepare_stage = package_text.find("make-writable-tree.sh", copied_base)
        bundle_closure = package_text.index('"$BUNDLE/base/lib"', copied_base)
        if not copied_base < prepare_stage < bundle_closure:
            raise SystemExit(
                "package staging must make its copied immutable base writable "
                "before adding the runtime closure"
            )

        # Installed native-base objects are sealed read-only. The package
        # staging copy must be writable before the bundler adds dependencies.
        run("chmod", "-R", "a-w", str(bundle))
        run("bash", str(MAKE_WRITABLE_TREE), str(bundle))
        if not (bundle.stat().st_mode & stat.S_IWUSR):
            raise SystemExit("staging lib directory remained read-only after preparation")
        if not (consumer.stat().st_mode & stat.S_IWUSR):
            raise SystemExit("staging runtime library remained read-only after preparation")

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
            if path.read_bytes()[:4] != b"\x7fELF":
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

        # The final SearchKernel adapter is not stored inside base/lib, but its
        # DT_NEEDED set is part of the consumer runtime contract too. Prove the
        # bundler can close dependencies discovered only through an extra ELF.
        extra_name = "libsearchkernel_fixture_adapter_dep.so.1"
        (root / "extra.c").write_text(
            "int searchkernel_fixture_adapter_dep(void) { return 11; }\n",
            encoding="utf-8",
        )
        extra_dep = producer / extra_name
        compile_shared(cc, root / "extra.c", extra_dep, extra_name)
        (producer / "libsearchkernel_fixture_adapter_dep.so").symlink_to(
            extra_dep.name
        )

        (root / "adapter.c").write_text(
            "extern int searchkernel_fixture_adapter_dep(void);\n"
            "int searchkernel_fixture_adapter(void) { "
            "return searchkernel_fixture_adapter_dep(); }\n",
            encoding="utf-8",
        )
        adapter = root / "libsearchkernel_fixture_adapter.so.2"
        compile_shared(
            cc,
            root / "adapter.c",
            adapter,
            "libsearchkernel_fixture_adapter.so.2",
            "-L",
            str(producer),
            "-Wl,--no-as-needed",
            "-lsearchkernel_fixture_adapter_dep",
        )

        extra_before = subprocess.run(
            [sys.executable, str(CHECKER), str(bundle), str(adapter)],
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )
        if extra_before.returncode == 0 or extra_name not in extra_before.stderr:
            raise SystemExit(
                "checker did not expose adapter-only runtime dependency:\n"
                + extra_before.stderr
            )

        run(
            sys.executable,
            str(BUNDLER),
            str(bundle),
            "--extra-elf",
            str(adapter),
            "--search-dir",
            str(producer),
            env=env,
        )
        if not (bundle / extra_name).is_file():
            raise SystemExit("bundler did not copy adapter-only runtime dependency")

        extra_complete = subprocess.run(
            [sys.executable, str(CHECKER), str(bundle), str(adapter)],
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )
        if extra_complete.returncode != 0:
            raise SystemExit(
                "closure checker rejected adapter-complete runtime closure:\n"
                + extra_complete.stderr
            )

        # Cargo's adapter may carry a DT_NEEDED path relative to its own
        # executable origin. Follow that path from the adapter's directory,
        # including package-hidden directories such as bin/.searchkernel-native.
        origin_bin = root / "origin-bin"
        origin_private = origin_bin / ".searchkernel-native"
        origin_private.mkdir(parents=True)
        origin_name = "libsearchkernel_fixture_origin.so.1"
        origin_source = root / "origin.c"
        origin_source.write_text(
            "int searchkernel_fixture_origin(void) { return 13; }\n",
            encoding="utf-8",
        )
        origin_library = origin_private / origin_name
        compile_shared(
            cc,
            origin_source,
            origin_library,
            "$ORIGIN/.searchkernel-native/" + origin_name,
        )
        (origin_private / "libsearchkernel_fixture_origin.so").symlink_to(origin_name)
        (root / "origin-adapter.c").write_text(
            "extern int searchkernel_fixture_origin(void);\n"
            "int searchkernel_fixture_origin_adapter(void) { "
            "return searchkernel_fixture_origin(); }\n",
            encoding="utf-8",
        )
        origin_adapter = origin_bin / "libsearchkernel_fixture_origin_adapter.so.2"
        compile_shared(
            cc,
            root / "origin-adapter.c",
            origin_adapter,
            "libsearchkernel_fixture_origin_adapter.so.2",
            "-L",
            str(origin_private),
            "-Wl,--no-as-needed",
            "-lsearchkernel_fixture_origin",
        )
        origin_complete = subprocess.run(
            [sys.executable, str(CHECKER), str(bundle), str(origin_adapter)],
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )
        if origin_complete.returncode != 0:
            raise SystemExit(
                "closure checker rejected a resolvable origin-relative adapter dependency:\n"
                + origin_complete.stderr
            )
        run(
            sys.executable,
            str(BUNDLER),
            str(bundle),
            "--extra-elf",
            str(origin_adapter),
            "--search-dir",
            str(producer),
            env=env,
        )

    print("native runtime bundling regression: PASS")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
