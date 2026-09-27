#!/usr/bin/env python3
import json
import sys


def main() -> None:
    for path in sys.argv[1:]:
        with open(path, encoding="utf-8") as stream:
            data = json.load(stream)
        data.pop("adapter_static_link_closure", None)
        with open(path, "w", encoding="utf-8") as stream:
            json.dump(data, stream, sort_keys=True)
            stream.write("\n")


if __name__ == "__main__":
    main()
