#!/usr/bin/env python3
import json
import re
import sys

path, source_header, failpoints, sanitizer = sys.argv[1:]
with open(path, encoding="utf-8") as stream:
    manifest = json.load(stream)
with open(source_header, encoding="utf-8") as stream:
    header = stream.read()
matches = re.findall(r"^#define SK_NATIVE_ABI_VERSION ([0-9]+)U$", header, re.MULTILINE)
if len(matches) != 1:
    raise SystemExit(
        f"expected exactly one SK_NATIVE_ABI_VERSION in {source_header}, found {len(matches)}"
    )
manifest["native_abi_version"] = int(matches[0])
manifest["hardening_failpoints"] = failpoints == "1"
manifest["sanitizer"] = sanitizer
with open(path.replace("manifest.json", "native-build-fingerprint-v2.txt"), encoding="utf-8") as stream:
    manifest["native_build_fingerprint"] = next(
        line.split("=", 1)[1].strip()
        for line in stream
        if line.startswith("sha256=")
    )
with open(path, "w", encoding="utf-8") as stream:
    json.dump(manifest, stream, indent=2, sort_keys=True)
    stream.write("\n")
