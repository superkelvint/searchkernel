"""Validate the SDK manifest ABI without accepting ambiguous JSON authorities."""

import json
import sys
from pathlib import Path


def unique_object(pairs):
    result = {}
    for key, value in pairs:
        if key in result:
            raise ValueError(f"duplicate manifest key: {key}")
        result[key] = value
    return result


def validate(path, expected):
    manifest = json.loads(Path(path).read_text(encoding="utf-8"), object_pairs_hook=unique_object)
    if not isinstance(manifest, dict):
        raise ValueError("native SDK manifest must be an object")
    actual = manifest.get("native_abi_version")
    if type(actual) is not int or actual != expected:
        raise ValueError(f"native SDK manifest ABI {actual!r} does not match expected ABI {expected}")


if __name__ == "__main__":
    try:
        validate(sys.argv[1], int(sys.argv[2]))
    except (OSError, ValueError) as error:
        raise SystemExit(str(error)) from error
