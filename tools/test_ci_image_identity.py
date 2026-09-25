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
    composite = tmp / "base.Dockerfile"
    composite.write_text("FROM scratch\n", encoding="utf-8")

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

    composite_a = call("ci_image_composite_id", str(composite), digest_a, vespa_a).stdout.strip()
    composite_toolchain_changed = call(
        "ci_image_composite_id", str(composite), digest_b, vespa_a
    ).stdout.strip()
    composite_vespa_changed = call(
        "ci_image_composite_id", str(composite), digest_a, vespa_b
    ).stdout.strip()

    if composite_a == composite_toolchain_changed:
        raise SystemExit("composite identity ignored the toolchain image digest")
    if composite_a == composite_vespa_changed:
        raise SystemExit("composite identity ignored the Vespa image digest")

print("split CI image identity regression: PASS")
