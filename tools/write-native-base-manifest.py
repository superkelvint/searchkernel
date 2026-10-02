#!/usr/bin/env python3
"""Stamp a deterministic native-base manifest and toolchain marker."""

from __future__ import annotations

import argparse
import hashlib
import json
import pathlib


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("manifest", type=pathlib.Path)
    parser.add_argument("marker", type=pathlib.Path)
    parser.add_argument("provenance", type=pathlib.Path)
    parser.add_argument("build_environment")
    parser.add_argument("target")
    parser.add_argument("vespa_commit")
    parser.add_argument("generation", type=int)
    parser.add_argument("searchkernel_commit")
    args = parser.parse_args()
    provenance_hash = hashlib.sha256(args.provenance.read_bytes()).hexdigest()
    data = {
        "format_version": 3,
        "artifact_generation": args.generation,
        "build_environment": args.build_environment,
        "native_target": args.target,
        "vespa_commit": args.vespa_commit,
        "searchkernel_commit": args.searchkernel_commit,
        "adapter_static_link_closure": True,
        "runtime_provenance_sha256": provenance_hash,
    }
    for path in (args.manifest, args.marker):
        path.write_text(json.dumps(data, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
