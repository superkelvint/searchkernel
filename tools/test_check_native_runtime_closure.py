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

        (root / "base.c").write_text(
            "int searchkernel_fixture_base(void) { return 1; }\n",
            encoding="utf-8",
        )
        base = bundle / "libsearchkernel_fixture_base.so.1"
        run(
            cc,
            "-shared",
            "-fPIC",
            "-Wl,-soname,libsearchkernel_fixture_base.so.1",
            "-o",
            str(base),
            str(root / "base.c"),
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

        consumer = root / "libsearchkernel_fixture_consumer.so.1"
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
            [sys.executable, str(CHECKER), str(bundle), str(consumer)],
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
            [sys.executable, str(CHECKER), str(bundle), str(consumer)],
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )
        if fake.returncode == 0:
            raise SystemExit("checker accepted a non-ELF runtime dependency placeholder")
        (bundle / dep.name).unlink()

        shutil.copy2(dep, bundle / dep.name)
        complete = subprocess.run(
            [sys.executable, str(CHECKER), str(bundle), str(consumer)],
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )
        if complete.returncode != 0:
            raise SystemExit(
                "checker rejected complete bundled runtime closure:\n" + complete.stderr
            )

        # The DT_NEEDED name is the same in both producer variants, but the
        # consumer requires an exported symbol version. A name-only closure
        # check therefore accepts the bad payload even though the ELF loader
        # cannot load it.
        versioned_name = "libsearchkernel_fixture_versioned.so.1"
        (root / "versioned.c").write_text(
            "int searchkernel_fixture_versioned(void) { return 19; }\n",
            encoding="utf-8",
        )
        good_map = root / "good.map"
        good_map.write_text(
            "FIXTURE_1 { global: searchkernel_fixture_versioned; };\n",
            encoding="utf-8",
        )
        bad_map = root / "bad.map"
        bad_map.write_text(
            "FIXTURE_2 { global: searchkernel_fixture_versioned; };\n",
            encoding="utf-8",
        )
        good = producer / versioned_name
        run(
            cc,
            "-shared",
            "-fPIC",
            "-Wl,-soname," + versioned_name,
            "-Wl,--version-script=" + str(good_map),
            "-o",
            str(good),
            str(root / "versioned.c"),
        )
        (producer / "libsearchkernel_fixture_versioned.so").symlink_to(versioned_name)
        versioned_consumer_source = root / "versioned_consumer.c"
        versioned_consumer_source.write_text(
            "extern int searchkernel_fixture_versioned(void);\n"
            "int searchkernel_fixture_versioned_consumer(void) { "
            "return searchkernel_fixture_versioned(); }\n",
            encoding="utf-8",
        )
        versioned_consumer = bundle / "libsearchkernel_fixture_versioned_consumer.so"
        run(
            cc,
            "-shared",
            "-fPIC",
            "-L" + str(producer),
            "-Wl,-rpath,$ORIGIN",
            "-Wl,--no-as-needed",
            "-lsearchkernel_fixture_versioned",
            "-o",
            str(versioned_consumer),
            str(versioned_consumer_source),
        )

        shutil.copy2(good, bundle / versioned_name)
        good_check = subprocess.run(
            [sys.executable, str(CHECKER), str(bundle), str(versioned_consumer)],
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )
        if good_check.returncode != 0:
            raise SystemExit(
                "checker rejected the correctly versioned fixture:\n" + good_check.stderr
            )

        bad = producer / "bad" / versioned_name
        bad.parent.mkdir()
        run(
            cc,
            "-shared",
            "-fPIC",
            "-Wl,-soname," + versioned_name,
            "-Wl,--version-script=" + str(bad_map),
            "-o",
            str(bad),
            str(root / "versioned.c"),
        )
        soname = subprocess.run(
            ["readelf", "-d", str(good)],
            check=True,
            text=True,
            stdout=subprocess.PIPE,
        ).stdout.split("Library soname: [", 1)[1].split("]", 1)[0]
        bad_soname = subprocess.run(
            ["readelf", "-d", str(bad)],
            check=True,
            text=True,
            stdout=subprocess.PIPE,
        ).stdout.split("Library soname: [", 1)[1].split("]", 1)[0]
        if soname != bad_soname:
            raise SystemExit("good and bad fixtures must have the same ELF SONAME")
        shutil.copy2(bad, bundle / versioned_name)
        needed = subprocess.run(
            ["readelf", "-d", str(versioned_consumer)],
            check=True,
            text=True,
            stdout=subprocess.PIPE,
        ).stdout
        if versioned_name not in needed or not (bundle / versioned_name).is_file():
            raise SystemExit("filename-only closure would not consider the fixture dependency present")
        bad_check = subprocess.run(
            [sys.executable, str(CHECKER), str(bundle), str(versioned_consumer)],
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )
        if bad_check.returncode == 0:
            raise SystemExit(
                "filename-only closure accepted a same-SONAME DSO without the required symbol version"
            )
        if "version" not in bad_check.stderr.lower():
            raise SystemExit(
                "bad symbol-version fixture rejected for the wrong reason:\n" + bad_check.stderr
            )

    print("native runtime closure regression: PASS")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
