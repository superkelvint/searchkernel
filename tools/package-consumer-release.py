#!/usr/bin/env python3
"""Build the supported Linux x86-64 consumer artifact set plus provenance/checksums."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import shutil
import subprocess
import sys
import tarfile
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
VERSION = "0.1.0"


def run(*cmd: str, cwd: Path = ROOT, env: dict[str, str] | None = None) -> None:
    subprocess.run(cmd, cwd=cwd, env=env, check=True)


def sha(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def cargo_metadata() -> dict[str, object]:
    return json.loads(
        subprocess.check_output(
            ["cargo", "metadata", "--no-deps", "--format-version", "1"],
            cwd=ROOT,
            text=True,
        )
    )


def target_dir(metadata: dict[str, object]) -> Path:
    configured = os.environ.get("CARGO_TARGET_DIR")
    if configured:
        path = Path(configured)
        return path if path.is_absolute() else ROOT / path
    return Path(str(metadata["target_directory"]))


def rust_source_paths(metadata: dict[str, object]) -> list[Path]:
    members = set(metadata["workspace_members"])
    paths: set[Path] = {
        Path("Cargo.toml"),
        Path("Cargo.lock"),
        Path("LICENSE"),
        Path("README.md"),
        Path("api"),
        Path("core/native/vespa"),
        Path("native-sdk.lock.json"),
        Path("upstream/vespa.commit"),
    }
    for optional in (Path("core/README.md"), Path(".cargo"), Path("rust-toolchain.toml")):
        if (ROOT / optional).exists():
            paths.add(optional)
    for package in metadata["packages"]:
        if package["id"] in members:
            manifest = Path(package["manifest_path"])
            paths.add(manifest.parent.relative_to(ROOT))
    for support in (
        "tools/resolve-native-sdk.sh",
        "tools/install-native-base-artifact.sh",
        "tools/native-base-release-selection.py",
        "tools/resolve-python.sh",
        "tools/resolve-rust.sh",
        "tools/native-build-fingerprint.py",
        "tools/check-native-sdk-hardening.sh",
    ):
        paths.add(Path(support))
    return sorted(paths, key=str)


def build_release_daemon(target: Path) -> tuple[Path, Path]:
    target.mkdir(parents=True, exist_ok=True)
    with tempfile.NamedTemporaryFile(
        prefix="searchkernel-release-cargo.", suffix=".json", delete=False
    ) as messages:
        messages_path = Path(messages.name)
    try:
        with messages_path.open("w", encoding="utf-8") as output:
            subprocess.run(
                [
                    "cargo",
                    "build",
                    "-p",
                    "searchkerneld",
                    "--release",
                    "--features",
                    "native",
                    "--message-format=json-render-diagnostics",
                ],
                cwd=ROOT,
                check=True,
                stdout=output,
            )
        adapter_lib = Path(
            subprocess.check_output(
                [
                    sys.executable,
                    str(ROOT / ".github/ci/resolve-native-adapter-lib.py"),
                    str(messages_path),
                    str(target),
                ],
                cwd=ROOT,
                text=True,
            ).strip()
        )
    finally:
        messages_path.unlink(missing_ok=True)

    daemon = target / "release" / "searchkerneld"
    if not daemon.is_file():
        raise SystemExit(f"release build did not produce searchkerneld: {daemon}")
    if not (adapter_lib / "libsearchkernel_vespa.so.2").is_file():
        raise SystemExit(
            "release build did not expose libsearchkernel_vespa.so.2 at "
            f"{adapter_lib}"
        )
    return daemon, adapter_lib


def clean_runtime_env() -> dict[str, str]:
    environment = os.environ.copy()
    for key in (
        "LD_LIBRARY_PATH",
        "SEARCHKERNEL_LOCAL_HOST",
        "SEARCHKERNEL_SDK_LIB",
        "SK_NATIVE_SDK",
        "SK_NATIVE_CACHE_DIR",
    ):
        environment.pop(key, None)
    return environment


def stage_runtime(daemon: Path, adapter_lib: Path, stage: Path) -> None:
    bindir = stage / "bin"
    libdir = stage / "lib"
    bindir.mkdir(parents=True)
    libdir.mkdir(parents=True)

    staged_daemon = bindir / "searchkerneld.real"
    shutil.copy2(daemon, staged_daemon)
    shutil.copy2(
        adapter_lib / "libsearchkernel_vespa.so.2",
        libdir / "libsearchkernel_vespa.so.2",
    )

    native_base = Path(
        subprocess.check_output(
            ["bash", str(ROOT / "tools/resolve-native-sdk.sh"), "path"],
            cwd=ROOT,
            text=True,
        ).strip()
    )
    base_lib = native_base / "lib"
    run(
        sys.executable,
        str(ROOT / "tools/bundle-native-runtime-closure.py"),
        str(libdir),
        "--search-dir",
        str(adapter_lib),
        "--search-dir",
        str(base_lib),
        "--extra-elf",
        str(staged_daemon),
    )

    run(
        sys.executable,
        str(ROOT / "tools/check-native-runtime-closure.py"),
        str(libdir),
        str(staged_daemon),
    )
    launcher = bindir / "searchkerneld"
    launcher.write_text(
        "#!/bin/sh\n"
        "set -eu\n"
        "bindir=$(CDPATH= cd -- \"$(dirname -- \"$0\")\" && pwd)\n"
        "root=$(dirname -- \"$bindir\")\n"
        "if [ -n \"${LD_LIBRARY_PATH:-}\" ]; then\n"
        "  export LD_LIBRARY_PATH=\"$root/lib:$LD_LIBRARY_PATH\"\n"
        "else\n"
        "  export LD_LIBRARY_PATH=\"$root/lib\"\n"
        "fi\n"
        "exec \"$bindir/searchkerneld.real\" \"$@\"\n",
        encoding="utf-8",
    )
    launcher.chmod(0o755)

    subprocess.run(
        [str(launcher), "--help"],
        check=True,
        env=clean_runtime_env(),
        stdout=subprocess.DEVNULL,
        stderr=subprocess.PIPE,
        text=True,
    )


def copy_runtime_into(package_root: Path, stage: Path) -> list[Path]:
    created: list[Path] = []
    for name in ("bin", "lib"):
        destination = package_root / name
        if destination.exists():
            raise SystemExit(
                f"refusing to overwrite pre-existing packaged runtime directory: {destination}"
            )
        shutil.copytree(stage / name, destination, copy_function=shutil.copy2)
        created.append(destination)
    return created


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--output", default="dist/release")
    args = parser.parse_args()
    out = (ROOT / args.output).resolve()
    shutil.rmtree(out, ignore_errors=True)
    out.mkdir(parents=True)

    metadata = cargo_metadata()

    # Rust embedded distribution is a complete versioned workspace snapshot.
    # Derive workspace members from Cargo metadata so a newly-added path crate
    # cannot silently be omitted from the consumer artifact.
    rust_bundle = out / f"searchkernel-rust-{VERSION}.tar.gz"
    with tarfile.open(rust_bundle, "w:gz") as archive:
        for relative in rust_source_paths(metadata):
            path = ROOT / relative
            if not path.exists():
                raise SystemExit(f"Rust consumer source is missing: {relative}")
            archive.add(path, arcname=str(relative))

    target = target_dir(metadata)
    daemon, adapter_lib = build_release_daemon(target)

    with tempfile.TemporaryDirectory(prefix="searchkernel-release-runtime.") as raw:
        stage = Path(raw)
        stage_runtime(daemon, adapter_lib, stage)

        daemon_bundle = out / f"searchkerneld-{VERSION}-linux-x86_64-glibc.tar.gz"
        with tarfile.open(daemon_bundle, "w:gz") as archive:
            archive.add(stage / "bin", arcname="bin")
            archive.add(stage / "lib", arcname="lib")

        # Managed-local packages carry the exact same daemon and native runtime
        # closure as the standalone distribution. A tiny POSIX launcher sets a
        # package-relative library path before execing the real ELF, so installed
        # packages require no SEARCHKERNEL_SDK_LIB/LD_LIBRARY_PATH.
        package_roots = (
            ROOT / "clients/python/searchkernel",
            ROOT / "clients/typescript",
        )
        created: list[Path] = []
        try:
            for package_root in package_roots:
                created.extend(copy_runtime_into(package_root, stage))
            run(
                sys.executable,
                "-m",
                "pip",
                "wheel",
                "--no-deps",
                "--no-build-isolation",
                "--wheel-dir",
                str(out),
                str(ROOT / "clients/python"),
            )
            run(
                "npm",
                "pack",
                "--pack-destination",
                str(out),
                cwd=ROOT / "clients/typescript",
            )
        finally:
            for path in reversed(created):
                shutil.rmtree(path, ignore_errors=True)

    commit = subprocess.check_output(
        ["git", "rev-parse", "HEAD"], cwd=ROOT, text=True
    ).strip()
    artifacts = []
    for path in sorted(out.iterdir()):
        if path.is_file() and path.name != "manifest.json":
            artifacts.append(
                {
                    "file": path.name,
                    "sha256": sha(path),
                    "bytes": path.stat().st_size,
                }
            )
    (out / "manifest.json").write_text(
        json.dumps(
            {
                "version": VERSION,
                "commit": commit,
                "platform": "linux-x86_64-glibc",
                "artifacts": artifacts,
            },
            indent=2,
        )
        + "\n"
    )
    print(out)


if __name__ == "__main__":
    main()
