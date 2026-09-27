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
manifest = json.loads((out / "manifest.json").read_text(encoding="utf-8"))
assert manifest["platform"] == "linux-x86_64-glibc"

for artifact in manifest["artifacts"]:
    path = out / artifact["file"]
    assert hashlib.sha256(path.read_bytes()).hexdigest() == artifact["sha256"]

# Assert artifacts are independently consumable and contain their runtime entry points.
rust = next(out.glob("searchkernel-rust-*.tar.gz"))
with tarfile.open(rust) as archive:
    names = set(archive.getnames())
    required = {
        "LICENSE",
        "core/crates/searchkernel/Cargo.toml",
        "core/crates/searchkernel-vespa-bindings/consumer_bundle.rs",
    }
    missing = sorted(required - names)
    assert not missing, f"Rust consumer bundle missing required entries: {missing}"
    assert (
        "tools/resolve-native-sdk.sh" not in names
    ), "Rust consumer bundle must use embedded native consumer support"

    with tempfile.TemporaryDirectory(prefix="searchkernel-rust-release-") as tempdir:
        extracted = Path(tempdir) / "source"
        archive.extractall(extracted)

        cargo = subprocess.check_output(
            ["bash", str(ROOT / "tools" / "resolve-rust.sh"), "cargo"],
            text=True,
        ).strip()
        environment = os.environ.copy()
        environment.pop("SK_NATIVE_SDK", None)
        environment.pop("SK_NATIVE_ABI_HEADER", None)
        environment.pop("SEARCHKERNEL_PACKAGED_SUPPORT_ROOT", None)
        environment["CARGO_TARGET_DIR"] = str(Path(tempdir) / "cargo-target")

        subprocess.run(
            [
                cargo,
                "metadata",
                "--no-deps",
                "--format-version",
                "1",
                "--manifest-path",
                str(extracted / "Cargo.toml"),
            ],
            check=True,
            env=environment,
            stdout=subprocess.DEVNULL,
        )
        subprocess.run(
            [
                cargo,
                "check",
                "-p",
                "searchkernel",
                "--features",
                "native",
                "--manifest-path",
                str(extracted / "Cargo.toml"),
            ],
            check=True,
            cwd=extracted,
            env=environment,
        )

wheel = next(out.glob("searchkernel-*.whl"))
with zipfile.ZipFile(wheel) as archive:
    assert any(name.endswith("searchkernel/__init__.py") for name in archive.namelist())

npm = next(out.glob("searchkernel-client-*.tgz"))
with tarfile.open(npm) as archive:
    assert "package/dist/node.js" in archive.getnames()

daemon = next(out.glob("searchkerneld-*.tar.gz"))
with tarfile.open(daemon) as archive:
    assert "bin/searchkerneld" in archive.getnames()

print("consumer release artifacts PASS")
