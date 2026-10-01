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
RUNTIME_MANIFEST_FORMAT = 1
RUNTIME_PACKAGE_GENERATION = 1
CONSUMER_RUNTIME_DIRS = {"bin", "lib"}



def run(*cmd: str, cwd: Path = ROOT, env: dict[str, str] | None = None) -> None:
    subprocess.run(cmd, cwd=cwd, env=env, check=True)


def sha(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def native_contract() -> dict[str, object]:
    data = json.loads((ROOT / "core/native/vespa/manifest.json").read_text(encoding="utf-8"))
    abi = data.get("native_abi_version")
    vespa_commit = data.get("vespa_commit")
    if not isinstance(abi, int) or not isinstance(vespa_commit, str):
        raise RuntimeError("native Vespa manifest lacks exact ABI/Vespa identity")
    return {"native_abi_version": abi, "vespa_commit": vespa_commit}


def native_target() -> str:
    configured = os.environ.get("SK_NATIVE_TARGET")
    if configured:
        return configured
    verbose = subprocess.check_output(["rustc", "-vV"], text=True)
    for line in verbose.splitlines():
        if line.startswith("host: "):
            return line.removeprefix("host: ")
    raise RuntimeError("could not determine native target from rustc -vV")


def runtime_identity(revision: str, platform: str) -> dict[str, object]:
    native = native_contract()
    material = {
        "searchkernel_version": VERSION,
        "searchkernel_revision": revision,
        "runtime_package_generation": RUNTIME_PACKAGE_GENERATION,
        "platform": platform,
        "native_target": native_target(),
        "build_environment": os.environ.get("SEARCHKERNEL_BUILD_ENV", "host"),
        **native,
    }
    canonical = json.dumps(material, sort_keys=True, separators=(",", ":")).encode()
    material["runtime_identity"] = "skrt-v1-" + hashlib.sha256(canonical).hexdigest()
    return material


def runtime_payload_digest(stage: Path) -> str:
    digest = hashlib.sha256()
    for path in sorted(p for p in stage.rglob("*") if p.is_file() and p.name != "manifest.json"):
        relative = path.relative_to(stage).as_posix().encode()
        digest.update(len(relative).to_bytes(8, "big"))
        digest.update(relative)
        data = path.read_bytes()
        digest.update(len(data).to_bytes(8, "big"))
        digest.update(data)
    return digest.hexdigest()


def assert_consumer_runtime_surface(stage: Path) -> None:
    extras = sorted(entry.name for entry in stage.iterdir() if entry.name not in CONSUMER_RUNTIME_DIRS | {"manifest.json"})
    if extras:
        raise RuntimeError(f"consumer runtime contains developer-only/unrecognized surface: {', '.join(extras)}")
    for forbidden in ("include", "cmake", "toolchain", "src", "source"):
        if (stage / forbidden).exists():
            raise RuntimeError(f"consumer runtime contains developer-only surface: {forbidden}")


def write_runtime_manifest(stage: Path, identity: dict[str, object]) -> dict[str, object]:
    assert_consumer_runtime_surface(stage)
    required = (stage / "bin/searchkerneld", stage / "bin/searchkerneld.real", stage / "lib/libsearchkernel_vespa.so.2")
    missing = [str(path.relative_to(stage)) for path in required if not path.is_file()]
    if missing:
        raise RuntimeError(f"consumer runtime missing required members: {', '.join(missing)}")
    manifest = {"format_version": RUNTIME_MANIFEST_FORMAT, **identity, "payload_sha256": runtime_payload_digest(stage)}
    (stage / "manifest.json").write_text(json.dumps(manifest, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    return manifest


def validate_runtime_manifest(stage: Path, expected: dict[str, object]) -> dict[str, object]:
    assert_consumer_runtime_surface(stage)
    path = stage / "manifest.json"
    try:
        manifest = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as exc:
        raise RuntimeError(f"invalid consumer runtime manifest: {exc}") from exc
    if manifest.get("format_version") != RUNTIME_MANIFEST_FORMAT:
        raise RuntimeError("runtime manifest format mismatch")
    for key, value in expected.items():
        if manifest.get(key) != value:
            raise RuntimeError(f"runtime identity mismatch for {key}: expected {value!r}, found {manifest.get(key)!r}")
    actual = runtime_payload_digest(stage)
    if manifest.get("payload_sha256") != actual:
        raise RuntimeError("runtime payload digest mismatch")
    return manifest


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


def packaged_rust_consumer_bundle() -> str:
    return (
        "// @generated for the publishable Rust consumer artifact; do not edit.\n"
        "pub const FILES: &[(&str, &[u8])] = &[];\n"
    )


def build_rust_crates(out: Path, descriptor: dict[str, object]) -> list[Path]:
    bindings_root = ROOT / "core/crates/searchkernel-vespa-bindings"
    runtime_descriptor_path = bindings_root / "runtime.json"
    consumer_bundle_path = bindings_root / "consumer_bundle.rs"
    if runtime_descriptor_path.exists():
        raise RuntimeError(f"refusing to overwrite source runtime descriptor: {runtime_descriptor_path}")
    original_consumer_bundle = consumer_bundle_path.read_text(encoding="utf-8")
    try:
        runtime_descriptor_path.write_text(
            json.dumps(descriptor, indent=2, sort_keys=True) + "\n", encoding="utf-8"
        )
        # The real publishable bindings crate carries this exact descriptor.
        # Its include list makes Cargo package the generated file even though it
        # is intentionally absent from the developer checkout.
        consumer_bundle_path.write_text(packaged_rust_consumer_bundle(), encoding="utf-8")
        with tempfile.TemporaryDirectory(prefix="searchkernel-cargo-package-") as raw:
            package_target = Path(raw)
            subprocess.run(
                [
                    "cargo", "package", "--workspace", "--exclude", "searchkerneld",
                    "--no-verify", "--locked", "--allow-dirty", "--target-dir", str(package_target),
                ],
                cwd=ROOT,
                check=True,
            )
            packaged = sorted((package_target / "package").glob("*.crate"))
            if not packaged:
                raise RuntimeError("cargo package did not produce any publishable Rust crates")
            artifacts: list[Path] = []
            for crate in packaged:
                target = out / crate.name
                shutil.copy2(crate, target)
                artifacts.append(target)
            return artifacts
    finally:
        consumer_bundle_path.write_text(original_consumer_bundle, encoding="utf-8")
        runtime_descriptor_path.unlink(missing_ok=True)


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
        "SK_NATIVE_ABI_HEADER",
        "SK_NATIVE_CACHE_DIR",
        "SEARCHKERNEL_PACKAGED_SUPPORT_ROOT",
    ):
        environment.pop(key, None)
    return environment


RUNTIME_PROBE_ARGUMENT = "--searchkernel-runtime-probe"


def probe_runtime_launcher(launcher: Path) -> None:
    result = subprocess.run(
        [str(launcher), RUNTIME_PROBE_ARGUMENT],
        check=False,
        env=clean_runtime_env(),
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )
    expected = f"unknown argument {RUNTIME_PROBE_ARGUMENT}"
    if result.returncode == 0 or expected not in result.stderr:
        diagnostic = (result.stderr or result.stdout).strip()
        raise RuntimeError(
            "packaged runtime probe did not reach the packaged daemon argument parser"
            f" (exit={result.returncode}): {diagnostic}"
        )


def stage_runtime(daemon: Path, adapter_lib: Path, stage: Path, identity: dict[str, object] | None = None) -> None:
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
    patchelf = shutil.which("patchelf")
    if patchelf is None:
        raise RuntimeError("consumer runtime packaging requires patchelf for relocatable native RPATH")
    run(patchelf, "--force-rpath", "--set-rpath", "$ORIGIN", str(libdir / "libsearchkernel_vespa.so.2"))
    cargo_adapter = bindir / ".searchkernel-native"
    cargo_adapter.mkdir()
    (cargo_adapter / "libsearchkernel_vespa.so.2").symlink_to(
        "../../lib/libsearchkernel_vespa.so.2"
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

    probe_runtime_launcher(launcher)
    if identity is not None:
        write_runtime_manifest(stage, identity)
        validate_runtime_manifest(stage, identity)


def runtime_descriptor(identity: dict[str, object], archive: Path) -> dict[str, object]:
    return {
        "format_version": RUNTIME_MANIFEST_FORMAT,
        **identity,
        "artifact_url": f"https://github.com/superkelvint/searchkernel/releases/download/runtime-{identity['runtime_identity']}/{archive.name}",
        "artifact_sha256": sha(archive),
    }


def stage_python_runtime_support(package_root: Path, descriptor: dict[str, object]) -> list[Path]:
    created = []
    runtime_module = package_root / "_runtime.py"
    shutil.copy2(ROOT / "tools/consumer_runtime.py", runtime_module)
    created.append(runtime_module)
    descriptor_path = package_root / "runtime.json"
    descriptor_path.write_text(json.dumps(descriptor, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    created.append(descriptor_path)
    return created


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
    manifest = package_root / "manifest.json"
    if manifest.exists():
        raise SystemExit(f"refusing to overwrite pre-existing runtime manifest: {manifest}")
    shutil.copy2(stage / "manifest.json", manifest)
    created.append(manifest)
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
    # The generated bindings consumer bundle owns native provisioning support,
    # so the source artifact must not depend on repository-only native helpers.
    target = target_dir(metadata)
    daemon, adapter_lib = build_release_daemon(target)
    commit = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip()
    platform = "linux-x86_64-glibc"
    identity = runtime_identity(commit, platform)

    with tempfile.TemporaryDirectory(prefix="searchkernel-release-runtime.") as raw:
        stage = Path(raw)
        stage_runtime(daemon, adapter_lib, stage, identity)

        daemon_bundle = out / f"searchkernel-runtime-{identity['runtime_identity']}-{platform}.tar.gz"
        with tarfile.open(daemon_bundle, "w:gz") as archive:
            archive.add(stage / "manifest.json", arcname="manifest.json")
            archive.add(stage / "bin", arcname="bin")
            archive.add(stage / "lib", arcname="lib")
        expanded_bytes = sum(path.stat().st_size for path in stage.rglob("*") if path.is_file())
        runtime_measurement = {
            "file": daemon_bundle.name,
            "runtime_identity": identity["runtime_identity"],
            "payload_sha256": json.loads((stage / "manifest.json").read_text())["payload_sha256"],
            "expanded_bytes": expanded_bytes,
            "compressed_bytes": daemon_bundle.stat().st_size,
        }

        # Managed-local packages carry the exact same daemon and native runtime
        # closure as the standalone distribution. A tiny POSIX launcher sets a
        # package-relative library path before execing the real ELF, so installed
        # packages require no SEARCHKERNEL_SDK_LIB/LD_LIBRARY_PATH.
        created: list[Path] = []
        try:
            descriptor = runtime_descriptor(identity, daemon_bundle)
            build_rust_crates(out, descriptor)
            created.extend(stage_python_runtime_support(ROOT / "clients/python/searchkernel", descriptor))
            ts_descriptor = ROOT / "clients/typescript/runtime.json"
            ts_descriptor.write_text(json.dumps(descriptor, indent=2, sort_keys=True) + "\n", encoding="utf-8")
            created.append(ts_descriptor)
            ruby_descriptor = ROOT / "clients/ruby/lib/searchkernel/runtime.json"
            ruby_descriptor.write_text(json.dumps(descriptor, indent=2, sort_keys=True) + "\n", encoding="utf-8")
            created.append(ruby_descriptor)
            run(
                sys.executable,
                "-m",
                "pip",
                "wheel",
                "--no-deps",
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
            run(
                "gem",
                "build",
                "searchkernel.gemspec",
                "--output",
                str(out / f"searchkernel-{VERSION}.gem"),
                cwd=ROOT / "clients/ruby",
            )
        finally:
            for path in reversed(created):
                if path.is_dir():
                    shutil.rmtree(path, ignore_errors=True)
                else:
                    path.unlink(missing_ok=True)

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
                "platform": platform,
                "runtime": {**identity, **runtime_measurement},
                "runtime_descriptor": descriptor,
                "artifacts": artifacts,
            },
            indent=2,
        )
        + "\n"
    )
    print(out)


if __name__ == "__main__":
    main()
