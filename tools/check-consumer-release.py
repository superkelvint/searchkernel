#!/usr/bin/env python3
import hashlib
import json
import os
import subprocess
import sys
import tarfile
import tempfile
import zipfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
out = Path(sys.argv[1])
manifest = json.loads((out / "manifest.json").read_text())
assert manifest["platform"] == "linux-x86_64-glibc"
for artifact in manifest["artifacts"]:
    path = out / artifact["file"]
    assert hashlib.sha256(path.read_bytes()).hexdigest() == artifact["sha256"]

def clean_runtime_env() -> dict[str, str]:
    env = os.environ.copy()
    for key in (
        "LD_LIBRARY_PATH",
        "SEARCHKERNEL_LOCAL_HOST",
        "SEARCHKERNEL_SDK_LIB",
        "SK_NATIVE_SDK",
        "SK_NATIVE_CACHE_DIR",
    ):
        env.pop(key, None)
    return env

def verify_daemon_tree(root: Path) -> None:
    daemon = root / "bin" / "searchkerneld"
    real_daemon = root / "bin" / "searchkerneld.real"
    libdir = root / "lib"
    assert daemon.is_file(), f"packaged daemon launcher missing: {daemon}"
    assert real_daemon.is_file(), f"packaged daemon ELF missing: {real_daemon}"
    assert (libdir / "libsearchkernel_vespa.so.2").is_file(), (
        f"packaged native adapter missing: {libdir / 'libsearchkernel_vespa.so.2'}"
    )
    subprocess.run(
        [
            sys.executable,
            str(ROOT / "tools" / "check-native-runtime-closure.py"),
            str(libdir),
            str(real_daemon),
        ],
        check=True,
        env=clean_runtime_env(),
    )
    # zipfile does not restore Unix executable bits when extracting wheels.
    # Package metadata is checked separately; restore the bits here so the
    # runtime probe exercises the packaged bytes/relative library layout.
    daemon.chmod(daemon.stat().st_mode | 0o111)
    real_daemon.chmod(real_daemon.stat().st_mode | 0o111)
    subprocess.run(
        [str(daemon), "--help"],
        check=True,
        env=clean_runtime_env(),
        stdout=subprocess.DEVNULL,
        stderr=subprocess.PIPE,
        text=True,
    )

with tempfile.TemporaryDirectory(prefix="searchkernel-release-check.") as raw:
    temp = Path(raw)

    rust = next(out.glob("searchkernel-rust-*.tar.gz"))
    rust_root = temp / "rust"
    rust_root.mkdir()
    with tarfile.open(rust) as archive:
        archive.extractall(rust_root)
    subprocess.run(
        ["cargo", "metadata", "--no-deps", "--format-version", "1"],
        cwd=rust_root,
        check=True,
        env=clean_runtime_env(),
        stdout=subprocess.DEVNULL,
    )

    daemon = next(out.glob("searchkerneld-*.tar.gz"))
    daemon_root = temp / "daemon"
    daemon_root.mkdir()
    with tarfile.open(daemon) as archive:
        archive.extractall(daemon_root)
    verify_daemon_tree(daemon_root)

    wheel = next(out.glob("searchkernel-*.whl"))
    wheel_root = temp / "wheel"
    wheel_root.mkdir()
    with zipfile.ZipFile(wheel) as archive:
        archive.extractall(wheel_root)
    assert (wheel_root / "searchkernel" / "__init__.py").is_file()
    verify_daemon_tree(wheel_root / "searchkernel")

    npm = next(out.glob("searchkernel-client-*.tgz"))
    npm_root = temp / "npm"
    npm_root.mkdir()
    with tarfile.open(npm) as archive:
        archive.extractall(npm_root)
    assert (npm_root / "package" / "dist" / "node.js").is_file()
    verify_daemon_tree(npm_root / "package")

print("consumer release artifacts PASS")
