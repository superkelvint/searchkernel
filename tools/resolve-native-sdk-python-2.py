#!/usr/bin/env python3
import re
import sys
from urllib.parse import urlparse

name = urlparse(sys.argv[1]).path.rsplit("/", 1)[-1]
match = re.match(r"^searchkernel-vespa-base-v([0-9]+)-.+\.tar\.gz$", name)
if not match:
    raise SystemExit(
        "SK_NATIVE_BASE_ARTIFACT_URL must use a canonical generation-bearing "
        "asset name, or SK_NATIVE_BASE_ARTIFACT_GENERATION must be set"
    )
print(match.group(1))
