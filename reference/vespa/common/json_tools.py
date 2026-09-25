#!/usr/bin/env python3
import argparse
import json
import sys


def load_json(path):
    if path == "-":
        return json.load(sys.stdin)
    with open(path, encoding="utf-8") as stream:
        return json.load(stream)


def command_field(args):
    value = load_json(args.path)
    for part in args.key.split("."):
        value = value[part]
    if isinstance(value, bool):
        print("true" if value else "false")
    elif value is None:
        print("null")
    else:
        print(value)


def command_canonicalize(args):
    value = load_json(args.path)
    print(json.dumps(value, sort_keys=True, separators=(",", ":")))


def command_cases(args):
    for case in load_json(args.path)["cases"]:
        print(*(case[key] for key in args.keys))


def command_m7_error_jobs(args):
    for case in load_json(args.path)["cases"]:
        for path in ("typed", "yql"):
            if path in case:
                print(case["id"], path, case[path + "_error"])


def main():
    parser = argparse.ArgumentParser()
    sub = parser.add_subparsers(dest="command", required=True)

    field = sub.add_parser("field")
    field.add_argument("path")
    field.add_argument("key")
    field.set_defaults(func=command_field)

    canonicalize = sub.add_parser("canonicalize")
    canonicalize.add_argument("path", nargs="?", default="-")
    canonicalize.set_defaults(func=command_canonicalize)

    cases = sub.add_parser("cases")
    cases.add_argument("path")
    cases.add_argument("keys", nargs="+")
    cases.set_defaults(func=command_cases)

    m7 = sub.add_parser("m7-error-jobs")
    m7.add_argument("path")
    m7.set_defaults(func=command_m7_error_jobs)

    args = parser.parse_args()
    args.func(args)


if __name__ == "__main__":
    main()
