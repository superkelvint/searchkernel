#!/usr/bin/env python3
from __future__ import annotations

import re
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]

BARE_TOOL_RE = re.compile(
    r"(?m)(?:^|[|;&]\s*)\s*(?:env\s+)?"
    r"(?:[A-Za-z_][A-Za-z0-9_]*=[^\s]+\s+)*"
    r"(python3|cargo|rustc|rustfmt|npm|node|flatc)\b(?!=)"
)

CRITICAL_SCRIPTS: dict[str, tuple[tuple[str, ...], tuple[str, ...]]] = {
    "dev": (
        (
            "tools/resolve-python.sh",
            "tools/resolve-rust.sh",
            "tools/resolve-node.sh",
            "tools/resolve-flatbuffers.sh",
            "tools/resolve-native-sdk.sh",
        ),
        ("python3", "cargo", "rustc", "rustfmt", "npm", "node", "flatc"),
    ),
    ".githooks/pre-commit": (
        ("tools/resolve-python.sh", "tools/resolve-rust.sh"),
        ("python3", "cargo", "rustc"),
    ),
    "tools/check-python-grpc.sh": (
        ("resolve-python.sh",),
        ("python3",),
    ),
    "tools/generate-python-grpc.sh": (
        ("resolve-python.sh",),
        ("python3",),
    ),
    "tools/generate-python-flatbuffers.sh": (
        ("resolve-python.sh", "resolve-flatbuffers.sh"),
        ("python3", "flatc"),
    ),
    "tools/run-python-native-e2e.sh": (
        ("resolve-python.sh",),
        ("python3",),
    ),
    "tools/check-searchkernel-rpc.sh": (
        ("resolve-rust.sh",),
        ("cargo", "rustc"),
    ),
    "tools/generate-searchkernel-rpc.sh": (
        ("resolve-rust.sh",),
        ("cargo", "rustc"),
    ),
    "tools/check-typescript-generated.sh": (
        ("resolve-node.sh",),
        ("npm", "node"),
    ),
    "tools/test-typescript-generation.sh": (
        ("resolve-node.sh",),
        ("npm", "node"),
    ),
    "tools/check-flatbuffers-bfbs.sh": (
        ("resolve-flatbuffers.sh",),
        ("flatc",),
    ),
    "tools/generate-flatbuffers-bfbs.sh": (
        ("resolve-flatbuffers.sh",),
        ("flatc",),
    ),
    "tools/generate-searchkernel-protocol.sh": (
        ("resolve-flatbuffers.sh", "resolve-rust.sh"),
        ("flatc", "rustfmt"),
    ),
    "tools/generate-typescript-flatbuffers.sh": (
        ("resolve-flatbuffers.sh",),
        ("flatc",),
    ),
    "tools/resolve-flatc.sh": (
        ("resolve-flatbuffers.sh",),
        ("flatc",),
    ),
    "tools/resolve-native-sdk.sh": (
        ("resolve-python.sh", "resolve-rust.sh"),
        ("python3", "rustc"),
    ),
    "reference/api-conformance/verify.sh": (
        ("resolve-python.sh", "resolve-rust.sh"),
        ("python3", "cargo", "rustc"),
    ),
    "reference/api-conformance/verify-fixtures.sh": (
        ("resolve-python.sh",),
        ("python3",),
    ),
}


def read(relative: str) -> str:
    path = ROOT / relative
    if not path.is_file():
        raise AssertionError(f"required toolchain authority file is missing: {relative}")
    return path.read_text(encoding="utf-8")


def function_body(shell: str, name: str) -> str:
    marker = f"{name}() {{"
    start = shell.find(marker)
    if start < 0:
        raise AssertionError(f"missing shell function {name}")
    rest = shell[start + len(marker) :]
    end = rest.find("\n}")
    if end < 0:
        raise AssertionError(f"could not delimit shell function {name}")
    return rest[:end]


class ToolchainAuthorityTests(unittest.TestCase):
    def test_canonical_resolvers_exist(self) -> None:
        for relative in (
            "tools/resolve-python.sh",
            "tools/resolve-rust.sh",
            "tools/resolve-node.sh",
            "tools/resolve-flatbuffers.sh",
            "tools/resolve-native-sdk.sh",
        ):
            self.assertTrue((ROOT / relative).is_file(), relative)

    def test_python_selection_does_not_shadow_other_toolchains(self) -> None:
        dev = read("dev")
        self.assertIn('PYTHON=$(bash "$ROOT/tools/resolve-python.sh")', dev)
        self.assertNotIn('PATH="$(dirname -- "$PYTHON"):$PATH"', dev)

    def test_critical_scripts_use_authorities_and_no_bare_tools(self) -> None:
        for relative, (required, forbidden) in CRITICAL_SCRIPTS.items():
            with self.subTest(relative=relative):
                text = read(relative)
                for resolver in required:
                    self.assertIn(resolver, text, f"{relative} must use {resolver}")

                for match in BARE_TOOL_RE.finditer(text):
                    tool = match.group(1)
                    if tool in forbidden:
                        line = text.count("\n", 0, match.start()) + 1
                        self.fail(
                            f"{relative}:{line}: direct '{tool}' invocation bypasses "
                            "the repository toolchain authority"
                        )

    def test_doctor_exercises_real_downstream_paths(self) -> None:
        dev = read("dev")

        generated = function_body(dev, "doctor_generated_protocol")
        self.assertIn("check_protocol", generated)

        typescript = function_body(dev, "doctor_typescript")
        self.assertIn("run_typescript_generation_check", typescript)

        rust = function_body(dev, "doctor_rust")
        self.assertIn('"$CARGO" metadata --no-deps --format-version 1', rust)
        self.assertIn('"$RUSTFMT" --version', rust)

        flatbuffers = function_body(dev, "doctor_flatbuffers_cpp")
        self.assertIn("check-flatbuffers-cpp.sh", flatbuffers)

        native = function_body(dev, "doctor_native_sdk")
        self.assertIn("native_sdk_path", native)

        native_path = function_body(dev, "native_sdk_path")
        self.assertIn("resolve-native-sdk.sh", native_path)
        self.assertNotIn("SK_NATIVE_SDK", native_path)
        self.assertIn("resolve_explicit_sdk", read("tools/resolve-native-sdk.sh"))

        flatc_wrapper = read("tools/resolve-flatc.sh")
        self.assertIn("resolve-flatbuffers.sh", flatc_wrapper)

        searchkerneld = function_body(dev, "doctor_searchkerneld")
        self.assertIn('"$CARGO" build -p searchkerneld --features native', searchkerneld)

    def test_doctor_system_tool_repair_matches_almalinux_package_surface(self) -> None:
        dev = read("dev")
        repair = function_body(dev, "doctor_repair_system_tools")

        self.assertIn("dnf install -y epel-release", repair)
        self.assertIn("dnf install -y ripgrep clang python3-clang", repair)
        self.assertNotIn(
            "dnf install -y ripgrep clang python3-clang libclang",
            repair,
        )

    def test_typescript_helpers_pin_node_for_npm_children(self) -> None:
        for relative in (
            "tools/check-typescript-generated.sh",
            "tools/test-typescript-generation.sh",
        ):
            text = read(relative)
            self.assertIn('node=$(bash "$root/tools/resolve-node.sh" node)', text)
            self.assertIn('PATH="$(dirname -- "$node"):$PATH"', text)

    def test_dev_repairs_every_canonical_toolchain(self) -> None:
        dev = read("dev")
        doctor = function_body(dev, "doctor")
        for repair in (
            "doctor_repair_rust",
            "doctor_repair_node",
            "doctor_repair_flatbuffers",
            "doctor_repair_python",
            "doctor_repair_native_sdk",
        ):
            self.assertRegex(
                doctor,
                rf"repair '[^']+' {repair}\b",
                f"doctor must invoke {repair}",
            )


if __name__ == "__main__":
    unittest.main()
