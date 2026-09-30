#!/usr/bin/env python3
import hashlib
import io
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
manifest = json.loads((out / "manifest.json").read_text(encoding="utf-8"))
assert manifest["platform"] == "linux-x86_64-glibc"
release_support = json.loads((ROOT / "docs" / "release-support.json").read_text())
wheel_platform_tag = release_support.get("python_wheel_platform_tag")
assert isinstance(wheel_platform_tag, str), (
    "docs/release-support.json must declare python_wheel_platform_tag"
)
expected_wheel_tag = f"py3-none-{wheel_platform_tag}"
for artifact in manifest["artifacts"]:
    path = out / artifact["file"]
    assert hashlib.sha256(path.read_bytes()).hexdigest() == artifact["sha256"]

def clean_runtime_env() -> dict[str, str]:
    env = os.environ.copy()
    for key in list(env):
        if key.startswith(("SEARCHKERNEL_", "SK_NATIVE_", "VESPA_")) or key in {
            "CARGO_MANIFEST_DIR",
            "CARGO_TARGET_DIR",
            "LD_LIBRARY_PATH",
            "LIBRARY_PATH",
            "NODE_PATH",
            "PYTHONPATH",
        }:
            env.pop(key, None)
    return env

def runtime_payload_digest(root: Path) -> str:
    digest = hashlib.sha256()
    for path in sorted(p for p in root.rglob("*") if p.is_file() and p.name != "manifest.json"):
        relative = path.relative_to(root).as_posix().encode()
        digest.update(len(relative).to_bytes(8, "big"))
        digest.update(relative)
        data = path.read_bytes()
        digest.update(len(data).to_bytes(8, "big"))
        digest.update(data)
    return digest.hexdigest()

def verify_daemon_tree(root: Path, expected_runtime: dict[str, object]) -> None:
    runtime_manifest = json.loads((root / "manifest.json").read_text(encoding="utf-8"))
    for key in ("runtime_identity", "searchkernel_version", "searchkernel_revision", "runtime_package_generation", "platform", "native_target", "build_environment", "native_abi_version", "vespa_commit"):
        assert runtime_manifest.get(key) == expected_runtime.get(key), (
            f"packaged runtime identity mismatch for {key}: "
            f"{runtime_manifest.get(key)!r} != {expected_runtime.get(key)!r}"
        )
    assert runtime_manifest.get("payload_sha256") == runtime_payload_digest(root), (
        "packaged runtime payload digest mismatch"
    )
    daemon = root / "bin" / "searchkerneld"
    real_daemon = root / "bin" / "searchkerneld.real"
    libdir = root / "lib"
    assert daemon.is_file(), f"packaged daemon launcher missing: {daemon}"
    assert real_daemon.is_file(), f"packaged daemon ELF missing: {real_daemon}"
    assert (root / "bin" / ".searchkernel-native" / "libsearchkernel_vespa.so.2").is_file(), (
        "packaged daemon is missing its origin-relative native adapter dependency"
    )
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
    probe_argument = "--searchkernel-runtime-probe"
    result = subprocess.run(
        [str(daemon), probe_argument],
        check=False,
        env=clean_runtime_env(),
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )
    assert result.returncode != 0
    assert f"unknown argument {probe_argument}" in result.stderr, (
        "packaged daemon did not reach its argument parser with clean runtime env: "
        f"exit={result.returncode} stderr={result.stderr!r}"
    )

with tempfile.TemporaryDirectory(prefix="searchkernel-release-check.") as raw:
    temp = Path(raw)

    crates = {path.name: path for path in out.glob("*.crate")}
    assert "searchkernel-0.1.0.crate" in crates, (
        "release is missing the publishable searchkernel Cargo crate"
    )
    bindings = crates.get("searchkernel-vespa-bindings-0.1.0.crate")
    assert bindings is not None, "release is missing the publishable Vespa bindings Cargo crate"
    with tarfile.open(bindings, "r:gz") as archive:
        descriptor_member = next(
            (member for member in archive.getmembers() if member.name.endswith("/runtime.json")),
            None,
        )
        bundle_member = next(
            (member for member in archive.getmembers() if member.name.endswith("/consumer_bundle.rs")),
            None,
        )
        assert descriptor_member is not None, (
            "publishable searchkernel-vespa-bindings .crate omits runtime.json"
        )
        assert bundle_member is not None, (
            "publishable searchkernel-vespa-bindings .crate omits consumer_bundle.rs"
        )
        packaged_descriptor = json.load(archive.extractfile(descriptor_member))
        packaged_support = archive.extractfile(bundle_member).read().decode("utf-8")
        assert packaged_descriptor == manifest["runtime_descriptor"], (
            "Cargo crate runtime descriptor differs from release metadata"
        )
        assert "tools/resolve-native-sdk.sh" not in packaged_support
        assert "core/native/vespa/build-sdk.sh" not in packaged_support

    daemon = next(out.glob("searchkernel-runtime-*.tar.gz"))
    daemon_root = temp / "daemon"
    daemon_root.mkdir()
    with tarfile.open(daemon) as archive:
        archive.extractall(daemon_root)
    verify_daemon_tree(daemon_root, manifest["runtime"])

    wheel = next(out.glob("searchkernel-*.whl"))
    assert wheel.name.endswith(f"-{expected_wheel_tag}.whl"), (
        "Python consumer wheel platform tag does not match release support policy: "
        f"expected *-{expected_wheel_tag}.whl, found {wheel.name}"
    )
    with zipfile.ZipFile(wheel) as archive:
        wheel_metadata_paths = [
            name for name in archive.namelist() if name.endswith(".dist-info/WHEEL")
        ]
        assert len(wheel_metadata_paths) == 1, (
            "Python consumer wheel must contain exactly one WHEEL metadata file"
        )
        wheel_metadata = archive.read(wheel_metadata_paths[0]).decode("utf-8")
    wheel_metadata_lines = set(wheel_metadata.splitlines())
    assert "Root-Is-Purelib: false" in wheel_metadata_lines, (
        "Python consumer wheel ships native runtime assets but is marked pure"
    )
    assert f"Tag: {expected_wheel_tag}" in wheel_metadata_lines, (
        "Python consumer wheel WHEEL metadata does not match release support policy: "
        f"expected Tag: {expected_wheel_tag}"
    )

    wheel_root = temp / "wheel"
    subprocess.run(
        [
            sys.executable,
            "-m",
            "pip",
            "install",
            "--no-deps",
            "--target",
            str(wheel_root),
            str(wheel),
        ],
        check=True,
        env=clean_runtime_env(),
        stdout=subprocess.DEVNULL,
    )
    assert (wheel_root / "searchkernel" / "__init__.py").is_file()
    verify_daemon_tree(wheel_root / "searchkernel", manifest["runtime"])

    npm = next(out.glob("searchkernel-client-*.tgz"))
    npm_root = temp / "npm"
    npm_root.mkdir()
    with tarfile.open(npm) as archive:
        archive.extractall(npm_root)
    assert (npm_root / "package" / "dist" / "node.js").is_file()
    verify_daemon_tree(npm_root / "package", manifest["runtime"])

    ruby_gems = sorted(out.glob("searchkernel-*.gem"))
    assert len(ruby_gems) == 1, f"expected exactly one packaged Ruby gem, found {ruby_gems}"
    gem = ruby_gems[0]
    ruby_files = sorted(
        [path.relative_to(ROOT / "clients/ruby").as_posix()
         for path in (ROOT / "clients/ruby/lib").rglob("*.rb")]
        + ["README.md"]
    )
    inspect_gem = r'''require "rubygems/package"
spec = Gem::Package.new(ARGV.fetch(0)).spec
expected = JSON.parse(ARGV.fetch(1))
abort "unexpected Ruby gem name/version" unless spec.name == "searchkernel" && spec.version.to_s == "0.1.0"
abort "Ruby gem contents differ from release sources" unless spec.files.sort == expected.sort
'''
    subprocess.run(
        ["ruby", "-rjson", "-e", inspect_gem, str(gem), json.dumps(ruby_files)],
        check=True,
        cwd=ROOT,
        env=clean_runtime_env(),
    )
    with tarfile.open(gem, "r:*") as package:
        data_member = package.extractfile("data.tar.gz")
        assert data_member is not None, "Ruby gem is missing data.tar.gz"
        with tarfile.open(fileobj=io.BytesIO(data_member.read()), mode="r:gz") as data:
            packaged_files = sorted(
                member.name.removeprefix("./")
                for member in data.getmembers()
                if member.isfile()
            )
        assert packaged_files == ruby_files, (
            "Ruby gem release contents do not match gemspec: "
            f"expected {ruby_files}, found {packaged_files}"
        )

print("consumer release artifacts PASS")
