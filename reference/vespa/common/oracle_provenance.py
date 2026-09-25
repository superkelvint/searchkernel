#!/usr/bin/env python3
import argparse
import json


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("provenance")
    parser.add_argument("pin")
    parser.add_argument("label")
    args = parser.parse_args()

    provenance = {}
    with open(args.provenance, encoding="utf-8") as stream:
        for line in stream:
            key, value = line.rstrip("\n").split("=", 1)
            provenance[key] = value

    with open(args.pin, encoding="utf-8") as stream:
        expected = json.load(stream)["commit"]

    if provenance.get("vespa_commit") != expected:
        raise SystemExit(f"{args.label} oracle not built from pinned Vespa revision")


if __name__ == "__main__":
    main()
