#!/usr/bin/env python3
import ast
import json
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
RUNTIME_SPEC = importlib.util.spec_from_file_location(
    "consumer_runtime", ROOT / "tools" / "consumer_runtime.py"
)
assert RUNTIME_SPEC and RUNTIME_SPEC.loader
RUNTIME = importlib.util.module_from_spec(RUNTIME_SPEC)
RUNTIME_SPEC.loader.exec_module(RUNTIME)


class ConsumerReleaseTests(unittest.TestCase):
    def test_ci_toolchain_provides_patchelf(self) -> None:
        dockerfile = (ROOT / "docker/searchkernel-ci-toolchain/Dockerfile").read_text(encoding="utf-8")
        self.assertRegex(dockerfile, r"(?m)^\s*patchelf \\$")

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

    def test_python_wheel_uses_lazy_runtime_descriptor_not_embedded_closure(self) -> None:
        project = tomllib.loads((ROOT / "clients/python/pyproject.toml").read_text(encoding="utf-8"))
        package_data = project["tool"]["setuptools"]["package-data"]["searchkernel"]
        self.assertIn("runtime.json", package_data)
        self.assertNotIn("lib/*", package_data)
        self.assertNotIn("bin/*", package_data)

    def test_node_and_ruby_packages_do_not_embed_runtime_closure(self) -> None:
        node = __import__("json").loads((ROOT / "clients/typescript/package.json").read_text())
        self.assertNotIn("bin", node["files"]); self.assertNotIn("lib", node["files"]); self.assertIn("runtime.json", node["files"])
        gemspec = (ROOT / "clients/ruby/searchkernel.gemspec").read_text()
        self.assertIn('"lib/**/*.json"', gemspec)

    def test_python_runtime_descriptor_uses_exact_artifact_digest(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            archive = Path(raw) / "runtime.tar.gz"
            archive.write_bytes(b"runtime artifact")
            identity = {"runtime_identity": "skrt-v1-test", "platform": "linux-x86_64-glibc"}
            descriptor = MODULE.runtime_descriptor(identity, archive)
            self.assertEqual(descriptor["artifact_sha256"], MODULE.sha(archive))
            self.assertIn("skrt-v1-test", descriptor["artifact_url"])

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
                self.assertIn(
                    "core/crates/searchkernel-vespa-bindings/src/consumer_runtime.rs", names
                )
                self.assertNotIn("tools/resolve-native-sdk.sh", names)

    def test_rust_bundle_carries_exact_consumer_runtime_descriptor(self) -> None:
        with tempfile.TemporaryDirectory() as tempdir:
            bundle = Path(tempdir) / "searchkernel-rust-test.tar.gz"
            descriptor = {"runtime_identity": "skrt-v1-" + "a" * 64, "artifact_sha256": "b" * 64}
            MODULE.build_rust_bundle(bundle, descriptor)
            with tarfile.open(bundle) as archive:
                member = archive.extractfile("core/crates/searchkernel-vespa-bindings/runtime.json")
                self.assertIsNotNone(member)
                self.assertEqual(json.loads(member.read()), descriptor)
                bundle = archive.extractfile("core/crates/searchkernel-vespa-bindings/consumer_bundle.rs")
                self.assertIsNotNone(bundle)
                packaged_support = bundle.read().decode("utf-8")
                self.assertNotIn("tools/consumer_runtime.py", packaged_support)
                self.assertNotIn("tools/resolve-native-sdk.sh", packaged_support)
                self.assertNotIn("core/native/vespa/build-sdk.sh", packaged_support)
        build_script = (ROOT / "core/crates/searchkernel-vespa-bindings/build.rs").read_text(encoding="utf-8")
        self.assertIn("consumer_runtime::ensure_runtime", build_script)
        packaged_resolver = build_script.split("fn configure_packaged_consumer_runtime(", 1)[1].split("\n}", 1)[0]
        self.assertNotIn("consumer_runtime.py", packaged_resolver)
        self.assertFalse((ROOT / "core/crates/searchkernel-vespa-bindings/runtime.json").exists())

    def test_rust_packaged_build_resolves_runtime_without_python(self) -> None:
        bindings = ROOT / "core/crates/searchkernel-vespa-bindings"
        descriptor_path = bindings / "runtime.json"
        self.assertFalse(descriptor_path.exists())
        conformance = json.loads((ROOT / "build/consumer-runtime/conformance.json").read_text())
        vector = conformance["vectors"][0]
        identity = {
            **vector["identity_material"],
            "runtime_identity": vector["runtime_identity"],
        }
        with tempfile.TemporaryDirectory() as tempdir:
            base = Path(tempdir)
            runtime = base / "runtime"
            (runtime / "bin").mkdir(parents=True)
            (runtime / "lib").mkdir()
            for relative, data in (
                ("bin/searchkerneld", b"launcher"),
                ("bin/searchkerneld.real", b"daemon"),
                ("lib/libsearchkernel_vespa.so.2", b"adapter"),
            ):
                path = runtime / relative
                path.write_bytes(data)
                if relative.startswith("bin/"):
                    path.chmod(0o755)
            manifest = {
                "format_version": 1,
                **identity,
                "payload_sha256": RUNTIME.payload_digest(runtime),
            }
            (runtime / "manifest.json").write_text(json.dumps(manifest))
            archive = base / RUNTIME.expected_artifact_filename(identity)
            descriptor = {
                "format_version": 1,
                **identity,
                "artifact_url": archive.as_uri(),
                "artifact_sha256": "a" * 64,
            }
            descriptor_path.write_text(json.dumps(descriptor))
            environment = os.environ.copy()
            environment["SEARCHKERNEL_RUNTIME_DIR"] = str(runtime)
            try:
                result = subprocess.run(
                    ["cargo", "check", "-p", "searchkernel-vespa-bindings", "--features", "native"],
                    cwd=ROOT,
                    env=environment,
                    capture_output=True,
                    text=True,
                    timeout=180,
                )
                self.assertEqual(result.returncode, 0, result.stderr)
            finally:
                descriptor_path.unlink(missing_ok=True)

    def test_publishable_rust_crates_pin_internal_dependency_versions(self) -> None:
        for manifest in (ROOT / "core/crates").glob("*/Cargo.toml"):
            data = tomllib.loads(manifest.read_text(encoding="utf-8"))
            for section in ("dependencies", "dev-dependencies", "build-dependencies"):
                for name, dependency in data.get(section, {}).items():
                    if name.startswith("searchkernel-") and isinstance(dependency, dict) and "path" in dependency:
                        self.assertEqual(dependency.get("version"), "=0.1.0", f"{manifest}: {name}")

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
