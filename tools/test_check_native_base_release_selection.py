#!/usr/bin/env python3
"""Regression coverage for native-base release readiness selection."""

import json
import os
import subprocess
import sys
import tempfile


ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
SELECTOR = os.path.join(ROOT, "tools", "native-base-release-selection.py")
BUILD_ENV = "ci"
TARGET = "x86_64-unknown-linux-gnu"
COMMIT = "a" * 40


def release(
    generation,
    draft=False,
    prerelease=False,
    published=True,
    archive=True,
    checksum=True,
    archive_state="uploaded",
    checksum_state="uploaded",
    archive_size=123,
    checksum_size=64,
):
    tag = "vespa-base-v{}-{}-{}-{}".format(
        generation, BUILD_ENV, TARGET, COMMIT
    )
    asset = "searchkernel-vespa-base-v{}-{}-{}-{}.tar.gz".format(
        generation, BUILD_ENV, TARGET, COMMIT
    )
    assets = []
    if archive:
        assets.append({"name": asset, "state": archive_state, "size": archive_size})
    if checksum:
        assets.append(
            {"name": asset + ".sha256", "state": checksum_state, "size": checksum_size}
        )
    return {
        "tag_name": tag,
        "draft": draft,
        "prerelease": prerelease,
        "published_at": "2026-09-24T00:00:00Z" if published else None,
        "assets": assets,
    }


def select(releases):
    tmp = tempfile.mkdtemp(prefix="searchkernel-release-selection.")
    try:
        fixture = os.path.join(tmp, "releases.json")
        with open(fixture, "w", encoding="utf-8") as stream:
            json.dump(releases, stream)
        output = subprocess.check_output(
            [
                sys.executable,
                SELECTOR,
                BUILD_ENV,
                TARGET,
                COMMIT,
                "--releases-file",
                fixture,
            ],
            universal_newlines=True,
        )
        return int(output.strip())
    finally:
        import shutil

        shutil.rmtree(tmp)


def main():
    releases = [
        release(1),
        release(2, draft=True, archive=False, checksum=True),
        release(3, prerelease=True),
        release(4, archive=True, checksum=False),
    ]
    assert select(releases) == 1

    releases[1] = release(2)
    assert select(releases) == 2

    releases.append(release(5, draft=True))
    releases.append(release(6, archive_state="new"))
    releases.append(release(7, checksum_state="new"))
    releases.append(release(8, archive_size=0))
    releases.append(release(9, checksum_size=0))
    assert select(releases) == 2

    releases.append(release(10))
    assert select(releases) == 10

    print("native-base release selection regression: PASS")
    return 0


if __name__ == "__main__":
    sys.exit(main())
