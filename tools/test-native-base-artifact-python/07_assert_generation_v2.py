#!/usr/bin/env python3
import json
import sys


def main() -> None:
    with open(sys.argv[1], encoding="utf-8") as stream:
        data = json.load(stream)
    assert data["artifact_generation"] == 2


if __name__ == "__main__":
    main()
