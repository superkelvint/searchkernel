#!/usr/bin/env python3
import argparse
import json


def load_json(path):
    with open(path, encoding="utf-8") as stream:
        return json.load(stream)


def expected_revision(args):
    print(load_json(args.pin)["commit"])


def validate(args):
    try:
        marker = load_json(args.marker)
    except (OSError, json.JSONDecodeError) as exc:
        raise SystemExit(
            f"invalid published Vespa provenance marker {args.marker}: {exc}"
        ) from exc

    expected = {
        "build_environment": args.build_environment,
        "native_target": args.native_target,
        "vespa_commit": args.vespa_commit,
    }
    for key, value in expected.items():
        if marker.get(key) != value:
            raise SystemExit(
                f"published Vespa provenance {key} mismatch: "
                f"expected {value!r}, found {marker.get(key)!r}"
            )

    generation = marker.get("artifact_generation")
    if not isinstance(generation, int) or generation < 1:
        raise SystemExit("published Vespa provenance has no valid artifact_generation")
    if marker.get("adapter_static_link_closure") is not True:
        raise SystemExit(
            "published Vespa provenance does not certify adapter static-link closure"
        )

    format_version = marker.get("format_version")
    legacy_generation_2 = format_version == 1 and generation == 2
    if format_version != 2 and not legacy_generation_2:
        raise SystemExit(
            "unsupported published Vespa provenance format: "
            f"format_version={format_version!r}, artifact_generation={generation!r}"
        )


def write_marker(args):
    data = {
        "format_version": args.format_version,
        "artifact_generation": args.generation,
        "adapter_static_link_closure": args.closure == "true",
        "build_environment": "verifier-regression",
        "native_target": "x86_64-unknown-linux-gnu",
        "vespa_commit": args.commit,
    }
    with open(args.marker, "w", encoding="utf-8") as stream:
        json.dump(data, stream, sort_keys=True)
        stream.write("\n")


def main():
    parser = argparse.ArgumentParser()
    sub = parser.add_subparsers(dest="command", required=True)

    expected = sub.add_parser("expected-revision")
    expected.add_argument("pin")
    expected.set_defaults(func=expected_revision)

    check = sub.add_parser("validate")
    check.add_argument("marker")
    check.add_argument("build_environment")
    check.add_argument("native_target")
    check.add_argument("vespa_commit")
    check.set_defaults(func=validate)

    writer = sub.add_parser("write-marker")
    writer.add_argument("marker")
    writer.add_argument("commit")
    writer.add_argument("closure")
    writer.add_argument("format_version", type=int)
    writer.add_argument("generation", type=int)
    writer.set_defaults(func=write_marker)

    args = parser.parse_args()
    args.func(args)


if __name__ == "__main__":
    main()
