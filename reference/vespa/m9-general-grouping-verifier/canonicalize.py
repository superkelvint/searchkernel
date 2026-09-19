#!/usr/bin/env python3
import json
import sys


def key_sort_key(group):
    key = group["key"]
    t = key["type"]
    if t == "string":
        return (0, key["value"])
    if t == "integer":
        return (1, int(key["value"]))
    raise SystemExit(f"unsupported group key type: {t}")


def canonical_group(group):
    return {
        "key": group["key"],
        "outputs": group.get("outputs", []),
        "groups": sorted(
            (canonical_group(child) for child in group.get("groups", [])),
            key=key_sort_key,
        ),
    }


def main():
    value = json.load(sys.stdin)
    out = {
        "id": value["id"],
        "groups": sorted(
            (canonical_group(group) for group in value.get("groups", [])),
            key=key_sort_key,
        ),
    }
    json.dump(out, sys.stdout, sort_keys=True, separators=(",", ":"))
    sys.stdout.write("\n")


if __name__ == "__main__":
    main()
