#!/usr/bin/env python3
from __future__ import annotations

import importlib.util
import sys
import tomllib
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
CHECKER = ROOT / "tools/check-python-dependencies.py"
PYPROJECT = ROOT / "clients/python/pyproject.toml"


def load_checker():
    spec = importlib.util.spec_from_file_location("searchkernel_python_dependencies", CHECKER)
    if spec is None or spec.loader is None:
        raise AssertionError(f"could not load {CHECKER}")
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


class PythonDependencyContractTests(unittest.TestCase):
    def test_runtime_protobuf_contract_matches_pyproject(self) -> None:
        data = tomllib.loads(PYPROJECT.read_text(encoding="utf-8"))
        dependencies = data["project"]["dependencies"]
        self.assertIn("protobuf>=7.35.1,<8", dependencies)

        checker = load_checker()
        self.assertEqual(checker.PROTOBUF_MIN, (7, 35, 1))
        self.assertEqual(checker.PROTOBUF_MAX_EXCLUSIVE, (8, 0, 0))

    def test_runtime_protobuf_accepts_compatible_minor_release(self) -> None:
        checker = load_checker()
        errors = checker.validate_versions(
            {
                "flatbuffers": "25.12.19",
                "grpcio": "1.84.0",
                "protobuf": "7.36.2",
            }
        )
        self.assertEqual(errors, [])

    def test_runtime_protobuf_rejects_outside_declared_range(self) -> None:
        checker = load_checker()
        for version in ("7.35.0", "8.0.0"):
            with self.subTest(version=version):
                errors = checker.validate_versions(
                    {
                        "flatbuffers": "25.12.19",
                        "grpcio": "1.84.0",
                        "protobuf": version,
                    }
                )
                self.assertTrue(
                    any("protobuf" in error for error in errors),
                    errors,
                )

    def test_codegen_remains_exact_pinned(self) -> None:
        checker = load_checker()
        compatible_runtime = {
            "flatbuffers": "25.12.19",
            "grpcio": "1.84.0",
            "protobuf": "7.36.2",
            "grpcio-tools": "1.84.0",
        }
        self.assertEqual(
            checker.validate_versions(compatible_runtime),
            [],
        )
        codegen_errors = checker.validate_versions(
            compatible_runtime,
            codegen=True,
        )
        self.assertTrue(
            any("protobuf" in error and "codegen" in error for error in codegen_errors),
            codegen_errors,
        )

        exact_codegen = dict(compatible_runtime)
        exact_codegen["protobuf"] = "7.35.1"
        self.assertEqual(
            checker.validate_versions(exact_codegen, codegen=True),
            [],
        )

    def test_flatbuffers_and_grpc_remain_exact(self) -> None:
        checker = load_checker()
        errors = checker.validate_versions(
            {
                "flatbuffers": "25.12.20",
                "grpcio": "1.84.1",
                "protobuf": "7.35.1",
            }
        )
        self.assertTrue(any("flatbuffers" in error for error in errors), errors)
        self.assertTrue(any("grpcio" in error for error in errors), errors)


if __name__ == "__main__":
    unittest.main()
