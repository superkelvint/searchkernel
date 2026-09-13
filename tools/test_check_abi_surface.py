#!/usr/bin/env python3
"""Executable regression tests for check_abi_surface.py."""

from __future__ import annotations

import importlib.util
import tempfile
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
CHECKER_PATH = ROOT / "tools" / "check_abi_surface.py"
SPEC = importlib.util.spec_from_file_location("check_abi_surface", CHECKER_PATH)
if SPEC is None or SPEC.loader is None:
    raise RuntimeError(f"could not load {CHECKER_PATH}")
CHECKER = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(CHECKER)


def check_header(contents: str) -> list[str]:
    with tempfile.TemporaryDirectory() as directory:
        path = Path(directory) / "fixture.h"
        path.write_text(contents, encoding="utf-8")
        return CHECKER.check(path)


def main() -> int:
    opaque_violations = check_header(
        """
        typedef struct sk_engine sk_engine;
        typedef struct sk_result sk_result;
        void sk_use(sk_engine *engine);
        sk_result *sk_result_create(void);
        """
    )
    assert opaque_violations == [], opaque_violations

    aggregate_violations = check_header(
        """
        struct exposed {
            int value;
        };
        struct returned {
            int value;
        };
        typedef struct exposed *exposed_ptr;
        void sk_bad(struct exposed *value);
        void sk_bad_alias(exposed_ptr value);
        struct returned sk_return(void);
        """
    )
    assert len(aggregate_violations) == 3, aggregate_violations
    assert "sk_bad" in aggregate_violations[0], aggregate_violations
    assert "exposed" in aggregate_violations[0], aggregate_violations
    assert "sk_bad_alias" in aggregate_violations[1], aggregate_violations
    assert "exposed" in aggregate_violations[1], aggregate_violations
    assert "sk_return" in aggregate_violations[2], aggregate_violations
    assert "returned" in aggregate_violations[2], aggregate_violations

    CHECKER.ALLOWLIST["sk_bad"] = "benchmark/abi.rs shows a gain, PR #482"
    try:
        allowlisted_violations = check_header(
            """
            struct exposed {
                int value;
            };
            void sk_bad(struct exposed *value);
            """
        )
        assert allowlisted_violations == [], allowlisted_violations
    finally:
        del CHECKER.ALLOWLIST["sk_bad"]

    print("ABI surface checker regression PASS")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
