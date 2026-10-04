#!/usr/bin/env python3
"""Return canonical generated changes from an isolated CI source snapshot.

The patch includes only declared generator outputs. It is emitted compressed
into the verification log so remote-only contributors can retrieve it through
the same authenticated exact-SHA workflow, without another publishing service.
"""
import base64
import gzip
from pathlib import Path
import subprocess


OUTPUTS = (
    "core/crates/searchkernel-protocol/src/generated",
    "core/crates/searchkernel-protocol/bfbs",
    "core/crates/searchkernel-protocol/src/lib.rs",
    "core/crates/searchkernel-protocol/src/protocol.rs",
    "clients/typescript/src/_wire/v1/generated",
    "clients/typescript/src/semantic.ts",
    "clients/python/searchkernel/_wire/v1/generated",
    "searchkerneld/src/generated",
    "core/crates/searchkernel-dispatch/tests/fixtures/portable/requests",
    "core/crates/searchkernel-dispatch/tests/fixtures/portable/responses",
)


def main():
    root = Path(__file__).resolve().parent.parent
    revision = subprocess.check_output(
        ["git", "rev-parse", "HEAD"], cwd=root, text=True
    ).strip()
    # Intent-to-add makes newly generated files part of the ordinary binary
    # diff. This index belongs to the CI invocation, never the caller's tree.
    subprocess.run(["git", "add", "--intent-to-add", "--", *OUTPUTS],
                   cwd=root, check=True)
    patch = subprocess.check_output(
        ["git", "diff", "--binary", "HEAD", "--", *OUTPUTS], cwd=root
    )
    encoded = base64.b64encode(gzip.compress(patch, mtime=0)).decode("ascii")
    print(f"PROTOCOL_GENERATION_BEGIN {revision}")
    for offset in range(0, len(encoded), 120):
        print(f"PROTOCOL_GENERATION_DATA {encoded[offset:offset + 120]}")
    print(f"PROTOCOL_GENERATION_END {revision}")


if __name__ == "__main__":
    main()
