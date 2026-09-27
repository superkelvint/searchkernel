#!/usr/bin/env python3
import importlib.util
import os
import subprocess
import tempfile
import unittest
from pathlib import Path
from unittest import mock

ROOT = Path(__file__).resolve().parents[1]
SPEC = importlib.util.spec_from_file_location(
    "package_consumer_release", ROOT / "tools" / "package-consumer-release.py"
)
assert SPEC and SPEC.loader
MODULE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MODULE)


class ConsumerReleaseTests(unittest.TestCase):
    def test_target_dir_uses_cargo_target_dir_override(self) -> None:
        with tempfile.TemporaryDirectory() as tempdir:
            target = Path(tempdir) / "cargo-target"
            with mock.patch.dict(os.environ, {"CARGO_TARGET_DIR": str(target)}):
                self.assertEqual(
                    MODULE.target_dir({"target_directory": str(ROOT / "target")}),
                    target,
                )

    def test_target_dir_defaults_to_cargo_metadata(self) -> None:
        with mock.patch.dict(os.environ, {}, clear=True):
            expected = ROOT / "target-from-metadata"
            self.assertEqual(
                MODULE.target_dir({"target_directory": str(expected)}),
                expected,
            )

    def test_clean_runtime_env_removes_repository_runtime_overrides(self) -> None:
        with mock.patch.dict(
            os.environ,
            {
                "LD_LIBRARY_PATH": "/workspace/native",
                "SEARCHKERNEL_LOCAL_HOST": "/workspace/searchkerneld",
                "SEARCHKERNEL_SDK_LIB": "/workspace/sdk/lib",
                "SK_NATIVE_SDK": "/workspace/sdk",
                "SK_NATIVE_CACHE_DIR": "/workspace/cache",
                "KEEP_ME": "yes",
            },
            clear=True,
        ):
            environment = MODULE.clean_runtime_env()
        self.assertEqual(environment.get("KEEP_ME"), "yes")
        for key in (
            "LD_LIBRARY_PATH",
            "SEARCHKERNEL_LOCAL_HOST",
            "SEARCHKERNEL_SDK_LIB",
            "SK_NATIVE_SDK",
            "SK_NATIVE_CACHE_DIR",
        ):
            self.assertNotIn(key, environment)

    def test_runtime_probe_requires_application_level_argument_error(self) -> None:
        launcher = Path("/tmp/packaged-searchkerneld")
        entered_program = subprocess.CompletedProcess(
            [str(launcher), "--searchkernel-runtime-probe"],
            1,
            stdout="",
            stderr="Error: \"unknown argument --searchkernel-runtime-probe\"\n",
        )
        with mock.patch.object(
            MODULE.subprocess, "run", return_value=entered_program
        ) as run:
            MODULE.probe_runtime_launcher(launcher)
        args, kwargs = run.call_args
        self.assertEqual(
            args[0], [str(launcher), "--searchkernel-runtime-probe"]
        )
        self.assertFalse(kwargs["check"])
        self.assertNotIn("LD_LIBRARY_PATH", kwargs["env"])

    def test_runtime_probe_rejects_dynamic_loader_failure(self) -> None:
        launcher = Path("/tmp/packaged-searchkerneld")
        loader_failure = subprocess.CompletedProcess(
            [str(launcher), "--searchkernel-runtime-probe"],
            127,
            stdout="",
            stderr=(
                "searchkerneld.real: error while loading shared libraries: "
                "libsearchkernel_vespa.so.2: cannot open shared object file\n"
            ),
        )
        with mock.patch.object(
            MODULE.subprocess, "run", return_value=loader_failure
        ):
            with self.assertRaisesRegex(
                RuntimeError, "did not reach the packaged daemon argument parser"
            ):
                MODULE.probe_runtime_launcher(launcher)


if __name__ == "__main__":
    unittest.main()
