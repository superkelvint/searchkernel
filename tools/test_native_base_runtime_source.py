#!/usr/bin/env python3
"""Reject stale SDK aliases and bundle the compatible declared runtime ELF."""

from __future__ import annotations

import json
import os
import pathlib
import shutil
import subprocess
import sys
import tempfile


ROOT = pathlib.Path(__file__).resolve().parents[1]
BUNDLER = ROOT / "tools" / "bundle-native-runtime-closure.py"
CHECKER = ROOT / "tools" / "check-native-runtime-closure.py"


def run(*args: str, env: dict[str, str] | None = None) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        args,
        check=True,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        env=env,
    )


def compile_shared(
    cc: str,
    source: pathlib.Path,
    output: pathlib.Path,
    soname: str,
    version_script: pathlib.Path,
) -> None:
    run(
        cc,
        "-shared",
        "-fPIC",
        f"-Wl,-soname,{soname}",
        f"-Wl,--version-script={version_script}",
        "-o",
        str(output),
        str(source),
    )


def main() -> int:
    cc = shutil.which("cc")
    if cc is None:
        raise SystemExit("cc is required for native-base runtime source regression")

    with tempfile.TemporaryDirectory(prefix="searchkernel-native-runtime-source.") as raw:
        root = pathlib.Path(raw)
        producer = root / "producer"
        dependency_dir = producer / "deps" / "lib64"
        good_dir = producer / "toolchain" / "lib64"
        old_sdk_dir = root / "old-sdk" / "lib"
        bundle = root / "installed-cache" / "objects" / "fixture" / "base" / "lib"
        for directory in (dependency_dir, good_dir, old_sdk_dir, bundle):
            directory.mkdir(parents=True)

        (root / "ffi.c").write_text(
            "int native_base_fixture_ffi(void) { return 42; }\n",
            encoding="utf-8",
        )
        good_map = root / "good.map"
        good_map.write_text(
            "FIXTURE_FFI_1 { global: native_base_fixture_ffi; };\n",
            encoding="utf-8",
        )
        bad_map = root / "bad.map"
        bad_map.write_text(
            "FIXTURE_FFI_2 { global: native_base_fixture_ffi; };\n",
            encoding="utf-8",
        )

        soname = "libffi.so.6"
        good = good_dir / soname
        bad = old_sdk_dir / soname
        compile_shared(cc, root / "ffi.c", good, soname, good_map)
        compile_shared(cc, root / "ffi.c", bad, soname, bad_map)
        (good_dir / "libffi.so").symlink_to(soname)
        (dependency_dir / soname).symlink_to(bad)

        # The previous filename-only test sees the needed name through this
        # symlink and would accept it, even though it points into an old SDK.
        (root / "adapter.c").write_text(
            "extern int native_base_fixture_ffi(void);\n"
            "int main(void) { return native_base_fixture_ffi() == 42 ? 0 : 1; }\n",
            encoding="utf-8",
        )
        adapter = bundle / "libsearchkernel_fixture_adapter"
        run(
            cc,
            "-L",
            str(good_dir),
            "-Wl,--no-as-needed",
            "-lffi",
            "-Wl,-rpath,$ORIGIN",
            "-o",
            str(adapter),
            str(root / "adapter.c"),
        )
        needed = run("readelf", "-d", str(adapter)).stdout
        if soname not in needed or not (dependency_dir / soname).is_file():
            raise SystemExit("fixture does not reproduce filename-only dependency presence")
        if (dependency_dir / soname).resolve() == good.resolve():
            raise SystemExit("stale dependency alias unexpectedly points to the good producer ELF")

        run(
            sys.executable,
            str(BUNDLER),
            str(bundle),
            "--search-dir",
            str(dependency_dir),
            "--search-dir",
            str(good_dir),
            "--source-root",
            f"producer={producer}",
        )

        packaged = bundle / soname
        if packaged.read_bytes() != good.read_bytes():
            raise SystemExit("runtime closure copied the stale SDK DSO instead of the compatible producer ELF")
        if packaged.read_bytes() == bad.read_bytes():
            raise SystemExit("runtime closure consumed the old SDK DSO")

        provenance = json.loads(
            (bundle / ".searchkernel-runtime-provenance.json").read_text(encoding="utf-8")
        )
        entry = next(
            (item for item in provenance["libraries"] if item.get("packaged_name") == soname),
            None,
        )
        expected_hash = run("sha256sum", str(good)).stdout.split()[0]
        if (
            entry is None
            or entry.get("source_identity") != "producer/toolchain/lib64/libffi.so.6"
            or entry.get("sha256") != expected_hash
            or "libsearchkernel_fixture_adapter" not in entry.get("required_by", [])
        ):
            raise SystemExit("runtime provenance omitted selected producer identity/hash/dependency")

        check = subprocess.run(
            [sys.executable, str(CHECKER), str(bundle), "--require-provenance"],
            check=False,
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )
        if check.returncode != 0:
            raise SystemExit("installed runtime closure failed loader verification:\n" + check.stderr)

        env = os.environ.copy()
        for variable in tuple(env):
            if variable.startswith("LD_"):
                env.pop(variable, None)
        smoke = subprocess.run(
            [str(adapter)],
            check=False,
            env=env,
        )
        if smoke.returncode != 0:
            raise SystemExit(f"installed base/lib runtime smoke failed: status {smoke.returncode}")

    print("native-base generic runtime source regression: PASS")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
