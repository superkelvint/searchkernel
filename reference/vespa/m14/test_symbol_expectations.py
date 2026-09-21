#!/usr/bin/env python3
"""Regression coverage for the M14 native-driver symbol contract."""

from pathlib import Path


ROOT = Path(__file__).resolve().parents[3]
VERIFY = ROOT / "reference/vespa/m14/verify-m14.sh"


def main() -> None:
    source = VERIFY.read_text(encoding="utf-8")
    if "sk_engine_delete" in source:
        raise SystemExit(
            "M14 verifier still requires removed sk_engine_delete; "
            "the frozen ABI exports sk_engine_remove"
        )

    expected = "sk_engine_remove"
    if source.count(expected) != 2:
        raise SystemExit(
            "M14 verifier must require sk_engine_remove for both native drivers"
        )

    print("M14 native-driver symbol expectations valid")


if __name__ == "__main__":
    main()
