#!/usr/bin/env python3
"""Executable regression tests for check_abi_surface.py."""

from __future__ import annotations

import importlib.util
import subprocess
import tempfile
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
CHECKER_PATH = ROOT / "tools" / "check_abi_surface.py"
SPEC = importlib.util.spec_from_file_location("check_abi_surface", CHECKER_PATH)
if SPEC is None or SPEC.loader is None:
    raise RuntimeError(f"could not load {CHECKER_PATH}")
CHECKER = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(CHECKER)

BASE_HEADER = """
#include <stdint.h>
#define SK_NATIVE_ABI_VERSION 12U

typedef struct sk_engine sk_engine;

int32_t sk_alpha(sk_engine *engine, uint32_t limit, const uint8_t *bytes);
uint64_t sk_beta(const char *name);
"""

BASE_LINKER_MAP = """
SEARCHKERNEL_2 {
    global:
        sk_alpha;
        sk_beta;
    local:
        *;
};
"""


def check_header(contents: str) -> list[str]:
    with tempfile.TemporaryDirectory() as directory:
        path = Path(directory) / "fixture.h"
        path.write_text(contents, encoding="utf-8")
        return CHECKER.check(path)


def check_contract(
    contents: str,
    *,
    linker_map: str = BASE_LINKER_MAP,
    baseline: str | None = None,
) -> list[str]:
    with tempfile.TemporaryDirectory() as directory:
        directory_path = Path(directory)
        header_path = directory_path / "fixture.h"
        map_path = directory_path / "fixture.map"
        header_path.write_text(contents, encoding="utf-8")
        map_path.write_text(linker_map, encoding="utf-8")
        baseline_path = None
        if baseline is not None:
            baseline_path = directory_path / "baseline.h"
            baseline_path.write_text(baseline, encoding="utf-8")
        return CHECKER.check_contract(
            header_path,
            linker_map_path=map_path,
            baseline_header_path=baseline_path,
        )


def generated_bindings(contents: str) -> str:
    with tempfile.TemporaryDirectory() as directory:
        directory_path = Path(directory)
        header_path = directory_path / "fixture.h"
        output_path = directory_path / "bindings.rs"
        header_path.write_text(contents, encoding="utf-8")
        result = subprocess.run(
            [
                "cargo",
                "run",
                "--locked",
                "--quiet",
                "--manifest-path",
                str(ROOT / "tools" / "check-native-bindings" / "Cargo.toml"),
                "--",
                str(header_path),
                str(output_path),
            ],
            cwd=ROOT,
            check=False,
            capture_output=True,
            text=True,
        )
        if result.returncode != 0:
            raise AssertionError(
                "bindgen fixture generation failed\n"
                f"stdout={result.stdout}\n"
                f"stderr={result.stderr}"
            )
        return output_path.read_text(encoding="utf-8")


def replace_once(contents: str, old: str, new: str) -> str:
    if contents.count(old) != 1:
        raise AssertionError(f"expected exactly one fixture occurrence of {old!r}")
    return contents.replace(old, new, 1)


def assert_same_version_signature_change_rejected(contents: str) -> None:
    violations = check_contract(contents, baseline=BASE_HEADER)
    assert any(
        "without increasing SK_NATIVE_ABI_VERSION" in violation
        for violation in violations
    ), violations


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

    assert check_contract(BASE_HEADER) == []

    mutations = [
        replace_once(BASE_HEADER, "uint32_t limit", "uint64_t limit"),
        replace_once(
            BASE_HEADER,
            "sk_engine *engine, uint32_t limit, const uint8_t *bytes",
            "sk_engine *engine, const uint8_t *bytes, uint32_t limit",
        ),
        replace_once(BASE_HEADER, "const uint8_t *bytes", "uint8_t *bytes"),
        replace_once(BASE_HEADER, ", const uint8_t *bytes", ""),
        replace_once(
            BASE_HEADER,
            "uint32_t limit,",
            "uint32_t limit, uint64_t offset,",
        ),
        replace_once(BASE_HEADER, "uint64_t sk_beta", "uint32_t sk_beta"),
    ]

    baseline_bindings = generated_bindings(BASE_HEADER)
    for mutated in mutations:
        assert_same_version_signature_change_rejected(mutated)
        assert generated_bindings(mutated) != baseline_bindings

    bumped = mutations[0].replace(
        "#define SK_NATIVE_ABI_VERSION 12U",
        "#define SK_NATIVE_ABI_VERSION 13U",
    )
    bumped_violations = check_contract(bumped, baseline=BASE_HEADER)
    assert not any(
        "without increasing SK_NATIVE_ABI_VERSION" in violation
        for violation in bumped_violations
    ), bumped_violations

    missing_symbol_map = BASE_LINKER_MAP.replace("        sk_beta;\n", "")
    missing_symbol_violations = check_contract(
        BASE_HEADER,
        linker_map=missing_symbol_map,
    )
    assert any(
        "sk_beta: declared in header but missing from linker map" in violation
        for violation in missing_symbol_violations
    ), missing_symbol_violations

    extra_symbol_map = BASE_LINKER_MAP.replace(
        "        sk_beta;\n",
        "        sk_beta;\n        sk_extra;\n",
    )
    extra_symbol_violations = check_contract(
        BASE_HEADER,
        linker_map=extra_symbol_map,
    )
    assert any(
        "sk_extra: exported by linker map but missing from header" in violation
        for violation in extra_symbol_violations
    ), extra_symbol_violations

    repository_violations = CHECKER.check_contract(
        ROOT / "core" / "native" / "vespa" / "searchkernel_vespa.h",
        linker_map_path=ROOT
        / "core"
        / "native"
        / "vespa"
        / "searchkernel_vespa.map",
    )
    assert repository_violations == [], repository_violations

    print("ABI surface checker regression PASS")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
