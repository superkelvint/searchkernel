#!/usr/bin/env python3
import json
import sys


def main() -> None:
    payload = {
        "format_version": 1,
        "artifact_generation": 2,
        "adapter_static_link_closure": True,
        "build_environment": "artifact-regression",
        "native_target": "x86_64-unknown-linux-gnu",
        "vespa_commit": "fixture-vespa-commit",
    }
    for path in sys.argv[1:]:
        with open(path, "w", encoding="utf-8") as stream:
            json.dump(payload, stream, sort_keys=True)
            stream.write("\n")


if __name__ == "__main__":
    main()
