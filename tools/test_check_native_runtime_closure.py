#!/usr/bin/env python3
"""Regression coverage for host-independent native runtime closure validation."""

import pathlib
import shutil
import subprocess
import sys
import tempfile


ROOT = pathlib.Path(__file__).resolve().parents[1]
CHECKER = ROOT / "tools" / "check-native-runtime-closure.py"


def run(*args, **kwargs):
    return subprocess.run(args, check=True, text=True, **kwargs)


def main():
    cc = shutil.which("cc")
    if cc is None:
        raise SystemExit("cc is required for native runtime closure regression")

    with tempfile.TemporaryDirectory(prefix="searchkernel-runtime-closure.") as raw:
        root = pathlib.Path(raw)
        producer = root / "producer"
        bundle = root / "bundle"
        producer.mkdir()
        bundle.mkdir()

        (root / "dep.c").write_text(
            "int searchkernel_fixture_dep(void) { return 7; }\n",
            encoding="utf-8",
        )
        (root / "consumer.c").write_text(
            "extern int searchkernel_fixture_dep(void);\n"
            "int searchkernel_fixture_consumer(void) { return searchkernel_fixture_dep(); }\n",
            encoding="utf-8",
        )

        dep = producer / "libsearchkernel_fixture_dep.so.1"
        run(
            cc,
            "-shared",
            "-fPIC",
            "-Wl,-soname,libsearchkernel_fixture_dep.so.1",
            "-o",
            str(dep),
            str(root / "dep.c"),
        )
        link_name = producer / "libsearchkernel_fixture_dep.so"
        link_name.symlink_to(dep.name)

        consumer = bundle / "libsearchkernel_fixture_consumer.so.1"
        run(
            cc,
            "-shared",
            "-fPIC",
            "-Wl,-soname,libsearchkernel_fixture_consumer.so.1",
            "-L",
            str(producer),
            "-Wl,-rpath,$ORIGIN",
            "-o",
            str(consumer),
            str(root / "consumer.c"),
            "-lsearchkernel_fixture_dep",
        )

        missing = subprocess.run(
            [sys.executable, str(CHECKER), str(bundle)],
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )
        if missing.returncode == 0:
            raise SystemExit("checker accepted a bundle with an unbundled DT_NEEDED dependency")
        if "libsearchkernel_fixture_dep.so.1" not in missing.stderr:
            raise SystemExit(
                "checker rejected incomplete bundle for the wrong reason:\n" + missing.stderr
            )

        # A filename alone is not closure. A GNU linker script, placeholder,
        # truncated file, or other non-ELF payload with the required SONAME
        # must not satisfy the runtime loader contract.
        (bundle / dep.name).write_text("INPUT(/host/lib/escape.so)\n", encoding="utf-8")
        fake = subprocess.run(
            [sys.executable, str(CHECKER), str(bundle)],
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )
        if fake.returncode == 0:
            raise SystemExit("checker accepted a non-ELF runtime dependency placeholder")
        (bundle / dep.name).unlink()

        shutil.copy2(dep, bundle / dep.name)
        complete = subprocess.run(
            [sys.executable, str(CHECKER), str(bundle)],
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )
        if complete.returncode != 0:
            raise SystemExit(
                "checker rejected complete bundled runtime closure:\n" + complete.stderr
            )

    print("native runtime closure regression: PASS")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
