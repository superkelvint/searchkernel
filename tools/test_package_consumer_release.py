#!/usr/bin/env python3
import ast
import importlib.util
import os
import subprocess
import tarfile
import tempfile
import tomllib
import unittest
from pathlib import Path
from unittest import mock

ROOT = Path(__file__).resolve().parents[1]
RELEASE_VERIFIER = ROOT / "tools" / "check-consumer-release.py"
SPEC = importlib.util.spec_from_file_location(
    "package_consumer_release", ROOT / "tools" / "package-consumer-release.py"
)
assert SPEC and SPEC.loader
MODULE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MODULE)


class ConsumerReleaseTests(unittest.TestCase):
    def test_packager_requires_canonical_image_entrypoint(self) -> None:
        with tempfile.TemporaryDirectory() as tempdir:
            missing_marker = Path(tempdir) / "native-validation-pass"
            with mock.patch.object(MODULE, "CANONICAL_IMAGE_MARKER", missing_marker):
                with mock.patch.dict(
                    os.environ, {"SEARCHKERNEL_IN_CI_IMAGE": "0"}, clear=False
                ):
                    with self.assertRaisesRegex(
                        SystemExit, r"\./dev test consumer-release"
                    ):
                        MODULE.require_canonical_release_environment()

            with mock.patch.dict(
                os.environ, {"SEARCHKERNEL_IN_CI_IMAGE": "1"}, clear=False
            ):
                MODULE.require_canonical_release_environment()

    def test_packaged_build_sdk_reexecs_through_bash_under_flock(self) -> None:
        with tempfile.TemporaryDirectory() as tempdir:
            root = Path(tempdir)
            script = root / "core/native/vespa/build-sdk.sh"
            script.parent.mkdir(parents=True)
            script.write_bytes((ROOT / "core/native/vespa/build-sdk.sh").read_bytes())
            script.chmod(0o644)

            fake_bin = root / "bin"
            fake_bin.mkdir()
            fake_flock = fake_bin / "flock"
            fake_flock.write_text(
                '#!/bin/sh\n[ "$2" = bash ] && [ "$3" = "$EXPECTED_SCRIPT" ]\n',
                encoding="utf-8",
            )
            fake_flock.chmod(0o755)
            environment = {
                "PATH": f"{fake_bin}:/usr/bin:/bin",
                "CARGO_TARGET_DIR": str(root / "target"),
                "EXPECTED_SCRIPT": str(script),
            }
            result = subprocess.run(
                ["bash", str(script)],
                env=environment,
                capture_output=True,
                text=True,
            )
            self.assertEqual(result.returncode, 0, result.stderr)

    def test_python_wheel_build_uses_declared_isolated_build_requirements(self) -> None:
        project = tomllib.loads(
            (ROOT / "clients/python/pyproject.toml").read_text(encoding="utf-8")
        )
        self.assertIn("wheel", project["build-system"]["requires"])
        self.assertNotIn(
            "--no-build-isolation",
            (ROOT / "tools/package-consumer-release.py").read_text(encoding="utf-8"),
        )

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
                "SK_NATIVE_ABI_HEADER": "/workspace/sdk/searchkernel_vespa.h",
                "SK_NATIVE_CACHE_DIR": "/workspace/cache",
                "SEARCHKERNEL_PACKAGED_SUPPORT_ROOT": "/workspace/support",
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
            "SK_NATIVE_ABI_HEADER",
            "SK_NATIVE_CACHE_DIR",
            "SEARCHKERNEL_PACKAGED_SUPPORT_ROOT",
        ):
            self.assertNotIn(key, environment)

    def test_runtime_adapter_requires_origin_only_rpath(self) -> None:
        adapter = Path("/tmp/libsearchkernel_vespa.so.2")
        readelf = (
            " 0x000000000000000f (RPATH)              "
            "Library rpath: [$ORIGIN]\n"
        )
        with mock.patch.object(
            MODULE.subprocess,
            "check_output",
            return_value=readelf,
        ) as check_output:
            MODULE.verify_runtime_adapter_relocatable(adapter)
        check_output.assert_called_once_with(
            ["readelf", "-d", str(adapter)],
            text=True,
        )

        leaked = (
            " 0x000000000000000f (RPATH)              "
            "Library rpath: [/opt/searchkernel/native-sdk-cache/base/lib]\n"
        )
        with mock.patch.object(MODULE.subprocess, "check_output", return_value=leaked):
            with self.assertRaisesRegex(RuntimeError, "must use only \\$ORIGIN"):
                MODULE.verify_runtime_adapter_relocatable(adapter)

    def test_release_relocates_adapter_only_after_daemon_link(self) -> None:
        packager = (ROOT / "tools/package-consumer-release.py").read_text(
            encoding="utf-8"
        )
        build_sdk = (ROOT / "core/native/vespa/build-sdk.sh").read_text(
            encoding="utf-8"
        )
        build_rs = (
            ROOT / "core/crates/searchkernel-vespa-bindings/build.rs"
        ).read_text(encoding="utf-8")
        self.assertNotIn("SK_NATIVE_ADAPTER_RPATH", packager)
        self.assertNotIn("SK_NATIVE_ADAPTER_RPATH", build_sdk)
        self.assertNotIn("SK_NATIVE_ADAPTER_RPATH", build_rs)
        build = packager.index('"cargo",\n                    "build"')
        relocate = packager.index("make_runtime_adapter_relocatable(staged_adapter)")
        self.assertLess(build, relocate)

    def test_release_adapter_relocation_rewrites_dynamic_string_in_place(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            adapter = Path(raw) / "libsearchkernel_vespa.so.2"
            original = "/opt/searchkernel/native-sdk-cache/ci/base/lib"
            adapter.write_bytes(b"prefix\0" + original.encode() + b"\0suffix")
            readelf = (
                " 0x000000000000000f (RPATH)              "
                f"Library rpath: [{original}]\n"
            )
            with mock.patch.object(
                MODULE.subprocess, "check_output", return_value=readelf
            ):
                MODULE.make_runtime_adapter_relocatable(adapter)
            payload = adapter.read_bytes()
            self.assertIn(b"$ORIGIN\0", payload)
            self.assertNotIn(original.encode(), payload)
            self.assertEqual(len(payload), len(b"prefix\0" + original.encode() + b"\0suffix"))

    def test_release_adapter_relocation_rejects_ambiguous_dynamic_string(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            adapter = Path(raw) / "libsearchkernel_vespa.so.2"
            original = "/opt/searchkernel/native-sdk-cache/ci/base/lib"
            adapter.write_bytes(
                original.encode() + b"\0middle\0" + original.encode() + b"\0"
            )
            readelf = (
                " 0x000000000000000f (RPATH)              "
                f"Library rpath: [{original}]\n"
            )
            with mock.patch.object(
                MODULE.subprocess, "check_output", return_value=readelf
            ):
                with self.assertRaisesRegex(RuntimeError, "appear exactly once"):
                    MODULE.make_runtime_adapter_relocatable(adapter)

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


    def test_runtime_manifest_has_exact_identity_and_payload_digest(self) -> None:
        with tempfile.TemporaryDirectory() as tempdir:
            stage = Path(tempdir) / "runtime"
            (stage / "bin").mkdir(parents=True)
            (stage / "lib").mkdir()
            (stage / "bin" / "searchkerneld").write_text("launcher", encoding="utf-8")
            (stage / "bin" / "searchkerneld.real").write_bytes(b"daemon")
            (stage / "lib" / "libsearchkernel_vespa.so.2").write_bytes(b"adapter")
            identity = MODULE.runtime_identity("deadbeef", "linux-x86_64-glibc")
            manifest = MODULE.write_runtime_manifest(stage, identity)
            self.assertEqual(manifest["format_version"], MODULE.RUNTIME_MANIFEST_FORMAT)
            self.assertEqual(manifest["searchkernel_version"], MODULE.VERSION)
            self.assertEqual(manifest["searchkernel_revision"], "deadbeef")
            native = MODULE.native_contract()
            self.assertEqual(manifest["native_abi_version"], native["native_abi_version"])
            self.assertEqual(manifest["vespa_commit"], native["vespa_commit"])
            self.assertEqual(manifest["platform"], "linux-x86_64-glibc")
            self.assertEqual(manifest["native_target"], MODULE.native_target())
            self.assertEqual(manifest["build_environment"], "host")
            self.assertRegex(manifest["runtime_identity"], r"^skrt-v1-")
            self.assertEqual(manifest["payload_sha256"], MODULE.runtime_payload_digest(stage))
            MODULE.validate_runtime_manifest(stage, identity)

    def test_runtime_manifest_rejects_identity_mismatch_and_payload_corruption(self) -> None:
        with tempfile.TemporaryDirectory() as tempdir:
            stage = Path(tempdir) / "runtime"
            (stage / "bin").mkdir(parents=True)
            (stage / "lib").mkdir()
            (stage / "bin" / "searchkerneld").write_text("launcher", encoding="utf-8")
            (stage / "bin" / "searchkerneld.real").write_bytes(b"daemon")
            (stage / "lib" / "libsearchkernel_vespa.so.2").write_bytes(b"adapter")
            identity = MODULE.runtime_identity("deadbeef", "linux-x86_64-glibc")
            MODULE.write_runtime_manifest(stage, identity)
            wrong = dict(identity)
            wrong["native_abi_version"] += 1
            with self.assertRaisesRegex(RuntimeError, "runtime identity mismatch"):
                MODULE.validate_runtime_manifest(stage, wrong)
            (stage / "lib" / "libsearchkernel_vespa.so.2").write_bytes(b"corrupt")
            with self.assertRaisesRegex(RuntimeError, "runtime payload digest mismatch"):
                MODULE.validate_runtime_manifest(stage, identity)

    def test_consumer_runtime_rejects_developer_only_surface(self) -> None:
        with tempfile.TemporaryDirectory() as tempdir:
            stage = Path(tempdir) / "runtime"
            (stage / "bin").mkdir(parents=True)
            (stage / "lib").mkdir()
            (stage / "include").mkdir()
            with self.assertRaisesRegex(RuntimeError, "developer-only"):
                MODULE.assert_consumer_runtime_surface(stage)

    def test_rust_bundle_is_self_contained_workspace(self) -> None:
        with tempfile.TemporaryDirectory() as tempdir:
            bundle = Path(tempdir) / "searchkernel-rust-test.tar.gz"
            MODULE.build_rust_bundle(bundle)
            with tarfile.open(bundle) as archive:
                names = set(archive.getnames())
                self.assertIn("LICENSE", names)
                self.assertIn("core/crates/searchkernel/Cargo.toml", names)
                self.assertIn("core/crates/searchkernel-rpc/Cargo.toml", names)
                self.assertIn("searchkerneld/Cargo.toml", names)
                self.assertIn(
                    "core/crates/searchkernel-vespa-bindings/consumer_bundle.rs", names
                )
                self.assertNotIn("tools/resolve-native-sdk.sh", names)

    def test_release_verifier_builds_extracted_native_searchkernel(self) -> None:
        verifier = RELEASE_VERIFIER.read_text(encoding="utf-8")
        tree = ast.parse(verifier)
        commands = []
        for node in ast.walk(tree):
            if not isinstance(node, ast.Call) or not node.args:
                continue
            func = node.func
            if not (
                isinstance(func, ast.Attribute)
                and isinstance(func.value, ast.Name)
                and func.value.id == "subprocess"
                and func.attr == "run"
                and isinstance(node.args[0], ast.List)
            ):
                continue
            commands.append(
                [element.value for element in node.args[0].elts if isinstance(element, ast.Constant)]
            )
        self.assertTrue(
            any(
                command[:5] == ["check", "-p", "searchkernel", "--features", "native"]
                for command in commands
            )
        )
        self.assertIn('"CARGO_TARGET_DIR"', verifier)
        self.assertIn('"SK_NATIVE_CACHE_DIR"', verifier)
        self.assertIn('"SEARCHKERNEL_BUILD_ENV"', verifier)



if __name__ == "__main__":
    unittest.main()
