#!/usr/bin/env python3
import os
import pathlib
import subprocess
import tempfile

ROOT = pathlib.Path(__file__).resolve().parents[1]
COMMON = ROOT / "tools/searchkernel-ci-image-common.sh"


def call(function: str, *args: str, check: bool = True) -> subprocess.CompletedProcess[str]:
    env = os.environ.copy()
    env["COMMON"] = str(COMMON)
    for index, value in enumerate(args):
        env[f"ARG{index}"] = value
    quoted_args = " ".join(f'"$ARG{index}"' for index in range(len(args)))
    return subprocess.run(
        [
            "bash",
            "-c",
            f'set -euo pipefail; source "$COMMON"; {function} {quoted_args}',
        ],
        env=env,
        text=True,
        capture_output=True,
        check=check,
    )


with tempfile.TemporaryDirectory(prefix="searchkernel-ci-image-identity.") as raw_tmp:
    tmp = pathlib.Path(raw_tmp)
    toolchain = tmp / "toolchain"
    toolchain.mkdir()
    (toolchain / "Dockerfile").write_text("FROM scratch\n", encoding="utf-8")
    environment = tmp / "base.Dockerfile"
    environment.write_text("FROM scratch\n", encoding="utf-8")

    digest_a = "registry.example/base@sha256:" + "a" * 64
    digest_b = "registry.example/base@sha256:" + "b" * 64
    vespa_a = "registry.example/vespa@sha256:" + "c" * 64
    vespa_b = "registry.example/vespa@sha256:" + "d" * 64

    toolchain_a = call("ci_image_toolchain_id", str(toolchain), digest_a).stdout.strip()
    toolchain_b = call("ci_image_toolchain_id", str(toolchain), digest_b).stdout.strip()
    if toolchain_a == toolchain_b:
        raise SystemExit("toolchain identity ignored the upstream base digest")

    mutable = call(
        "ci_image_toolchain_id",
        str(toolchain),
        "registry.example/base:latest",
        check=False,
    )
    if mutable.returncode == 0:
        raise SystemExit("toolchain identity accepted a mutable base tag")

    environment_a = call("ci_image_environment_id", str(environment), digest_a, vespa_a).stdout.strip()
    environment_toolchain_changed = call(
        "ci_image_environment_id", str(environment), digest_b, vespa_a
    ).stdout.strip()
    environment_vespa_changed = call(
        "ci_image_environment_id", str(environment), digest_a, vespa_b
    ).stdout.strip()

    if environment_a == environment_toolchain_changed:
        raise SystemExit("environment identity ignored the toolchain image digest")
    if environment_a == environment_vespa_changed:
        raise SystemExit("environment identity ignored the Vespa image digest")

    producer_root = tmp / "producer-root"
    (producer_root / "core/native/vespa").mkdir(parents=True)
    (producer_root / "core/native/vespa/manifest.json").write_text(
        '{"native_abi_version":13,"vespa_commit":"' + "1" * 40 + '"}\n',
        encoding="utf-8",
    )
    full_sdk = tmp / "full-sdk"
    full_sdk.mkdir()
    (full_sdk / "manifest.json").write_text(
        '{"native_abi_version":13,"native_build_fingerprint":"abc","vespa_commit":"' + "1" * 40 + '"}\n',
        encoding="utf-8",
    )
    fingerprint = full_sdk / "native-build-fingerprint-v2.txt"
    fingerprint.write_text("fingerprint-version=3\nsha256=abc\n", encoding="utf-8")

    producer_a = call(
        "ci_image_vespa_producer_id", str(producer_root), str(full_sdk)
    ).stdout.strip()
    fingerprint.write_text("fingerprint-version=3\nsha256=def\n", encoding="utf-8")
    producer_b = call(
        "ci_image_vespa_producer_id", str(producer_root), str(full_sdk)
    ).stdout.strip()
    if producer_a == producer_b:
        raise SystemExit("Vespa producer identity ignored the native SDK fingerprint")

print("split CI image identity regression: PASS")
