#!/usr/bin/env python3
import ast
import json
import subprocess
import sys
import unittest
from pathlib import Path
from unittest import mock

ROOT = Path(__file__).resolve().parents[1]


class QualificationPolicyTests(unittest.TestCase):
    def check_quickstart_documentation(self, document):
        read_text = Path.read_text
        documentation = ROOT / "docs" / "release-support.md"

        def read_with_document(path, *args, **kwargs):
            if path == documentation:
                return document
            return read_text(path, *args, **kwargs)

        with mock.patch.object(Path, "read_text", read_with_document):
            self.test_public_quickstarts_are_canonical_and_rendered()

    def test_quickstart_link_accepts_descriptive_display_labels(self):
        self.check_quickstart_documentation(
            "Use [generated release quickstarts](release-quickstart.md).\n"
        )

    def test_quickstart_link_rejects_wrong_destination(self):
        with self.assertRaises(AssertionError):
            self.check_quickstart_documentation(
                "Use [release-quickstart.md](unqualified-examples.md).\n"
            )

    def test_quickstart_link_rejects_missing_destination(self):
        with self.assertRaises(AssertionError):
            self.check_quickstart_documentation("No executable examples linked.\n")

    def test_gate_runs_in_consumer_container_without_producer_state(self):
        runner = (ROOT / ".github/ci/ci-image-runner.yml").read_text()
        marker = 'echo "==> Qualify packaged release in clean consumer"'
        self.assertIn(marker, runner)
        section = runner.split(marker, 1)[1].split("\n          fi", 1)[0]
        self.assertIn('-v "$ARTIFACT_DIR/consumer-release:/release:ro"', section)
        self.assertIn('-v "$ARTIFACT_DIR/consumer-release-tools:/qualification:ro"', section)
        self.assertIn("python3 /qualification/qualify-consumer-release.py /release", section)
        self.assertNotIn("GH_TOKEN", section)
        self.assertNotIn("GITHUB_TOKEN", section)
        for forbidden in (
            "$SOURCE_VOLUME",
            "$TARGET_VOLUME",
            "$NATIVE_VOLUME",
            "/work",
            "/cargo-target",
            "/opt/searchkernel/native-sdk-cache",
        ):
            self.assertNotIn(forbidden, section)

    def test_producer_exports_exact_release_artifacts_for_consumer_gate(self):
        runner = (ROOT / ".github/ci/ci-image-runner.yml").read_text()
        self.assertIn(
            "tools/package-consumer-release.py --output /artifacts/consumer-release",
            runner,
        )
        self.assertIn(
            "tools/check-consumer-release.py /artifacts/consumer-release",
            runner,
        )
        self.assertIn(
            "bash tools/stage-native-base-qualification-artifact.sh /artifacts/consumer-release-tools/native-base",
            runner,
        )
        self.assertIn(
            "cp tools/qualify-consumer-release.py /artifacts/consumer-release-tools/",
            runner,
        )
        self.assertIn(
            "cp -R examples/release-quickstart /artifacts/consumer-release-tools/quickstarts",
            runner,
        )
        self.assertIn(
            'run_phase client-e2e-qualification-policy "$PYTHON" tools/test_qualify_consumer_release.py',
            runner,
        )
        self.assertIn(
            'run_phase client-e2e-quickstart-docs "$PYTHON" tools/render-release-quickstarts.py --check',
            runner,
        )

    def test_gate_scrubs_repository_and_native_runtime_overrides(self):
        gate = (ROOT / "tools/qualify-consumer-release.py").read_text()
        self.assertIn('key.startswith(("SEARCHKERNEL_", "SK_NATIVE_", "VESPA_"))', gate)
        for variable in (
            "CARGO_MANIFEST_DIR",
            "CARGO_TARGET_DIR",
            "LD_LIBRARY_PATH",
            "LIBRARY_PATH",
            "NODE_PATH",
            "PYTHONPATH",
        ):
            self.assertIn(variable, gate)
        self.assertNotIn('"--no-index"', gate)
        rust_runner = gate.split("def run_rust_quickstart", 1)[1].split("\ndef ", 1)[0]
        self.assertIn("native_base_fixture()", rust_runner)
        self.assertIn('"SK_NATIVE_BASE_ARTIFACT_URL"', rust_runner)
        self.assertIn("native_base_archive.as_uri()", rust_runner)
        self.assertIn('"SK_NATIVE_CACHE_DIR"', rust_runner)
        self.assertNotIn("GH_TOKEN", rust_runner)
        self.assertNotIn("GITHUB_TOKEN", rust_runner)
        self.assertIn('"SEARCHKERNEL_BUILD_ENV"', rust_runner)

    def test_all_rust_consumer_probes_reuse_local_native_base_sidecar(self):
        gate = (ROOT / "tools/qualify-consumer-release.py").read_text()
        rust_probe = gate.split("def rust_embedded_probe", 1)[1].split("\n        def ", 1)[0]
        self.assertIn(
            "native_environment = run_rust_quickstart(rust_root, root)",
            rust_probe,
        )
        self.assertIn("env=native_environment", rust_probe)
        self.assertNotIn("GH_TOKEN", rust_probe)
        self.assertNotIn("GITHUB_TOKEN", rust_probe)

    def test_lifecycle_qualification_uses_explicit_cold_start_timeout(self):
        gate = (ROOT / "tools/qualify-consumer-release.py").read_text()
        for call in (
            'SearchKernel::Index.create(endpoint, fields: fields, rank_profiles: [profile], target: target, timeout: 120.0)',
            'SearchKernel::Index.create(endpoint, fields: fields, target: "release-a", timeout: 120.0)',
            'SearchKernel::Index.create(endpoint, fields: fields, target: "release-b", timeout: 120.0)',
            'SearchKernel::Index.open(endpoint, target: "release-a", timeout: 120.0)',
            'SearchKernel::Index.open(endpoint, target: "release-b", timeout: 120.0)',
            'SearchKernel::Index.open(endpoint, target: target, timeout: 120.0)',
        ):
            self.assertIn(call, gate)

    def test_packaged_supported_rows_own_clean_consumer_gate(self):
        authority = json.loads((ROOT / "docs/release-support.json").read_text())
        source = ast.parse((ROOT / "tools/qualify-consumer-release.py").read_text())
        registry = next(
            ast.literal_eval(node.value)
            for node in source.body
            if isinstance(node, ast.Assign)
            and any(
                isinstance(target, ast.Name)
                and target.id == "QUALIFICATION_PROBES"
                for target in node.targets
            )
        )
        supported = {
            f"{row['surface']}/{row['runtime']}": row
            for row in authority["supported"]
        }
        self.assertEqual(set(supported), set(registry))
        self.assertEqual(len(registry), len(set(registry.values())))
        self.assertTrue(all(isinstance(probe, str) and probe for probe in registry.values()))
        runners = next(
            {key.value for key in node.value.keys if isinstance(key, ast.Constant)}
            for node in ast.walk(source)
            if isinstance(node, ast.Assign)
            and any(
                isinstance(target, ast.Name) and target.id == "runners"
                for target in node.targets
            )
        )
        self.assertEqual(set(runners), set(registry.values()))
        self.assertEqual(
            {"tools/qualify-consumer-release.py dist/release"},
            {row.get("qualification") for row in supported.values()},
        )
        self.assertEqual(registry["ruby/connect"], "ruby_connect")
        main = next(
            node for node in source.body
            if isinstance(node, ast.FunctionDef) and node.name == "main"
        )
        self.assertTrue(any(
            isinstance(node, ast.For)
            and isinstance(node.iter, ast.Call)
            and isinstance(node.iter.func, ast.Attribute)
            and isinstance(node.iter.func.value, ast.Name)
            and node.iter.func.value.id == "QUALIFICATION_PROBES"
            and node.iter.func.attr == "items"
            for node in ast.walk(main)
        ), "main must dispatch each row from the structured support registry")
        self.assertTrue(any(
            isinstance(node, ast.Call)
            and isinstance(node.func, ast.Subscript)
            and isinstance(node.func.value, ast.Name)
            and node.func.value.id == "runners"
            for node in ast.walk(main)
        ), "registry entries must resolve through the runtime probe dispatch map")

    def test_consumer_release_stages_native_base_sidecar_locally(self):
        dev = (ROOT / "dev").read_text()
        self.assertIn(
            'stage-native-base-qualification-artifact.sh" "$release_root/qualification/native-base"',
            dev,
        )
        gate = (ROOT / "tools/qualify-consumer-release.py").read_text()
        self.assertIn('QUALIFICATION_ROOT / "native-base"', gate)
        self.assertIn("native_base_archive.as_uri()", gate)

    def test_local_native_base_sidecar_does_not_require_release_metadata(self):
        helper = (ROOT / "tools/stage-native-base-qualification-artifact.sh").read_text()
        self.assertIn("GENERATION=1", helper)
        self.assertNotIn(".searchkernel-native-base-artifact.json", helper)
        self.assertNotIn('marker.get("artifact_generation"', helper)
        self.assertIn('package-native-base-artifact.sh', helper)


    def test_qualifier_selects_versioned_runtime_artifact(self):
        gate = (ROOT / "tools/qualify-consumer-release.py").read_text()
        self.assertIn('artifact("searchkernel-runtime-", ".tar.gz")', gate)
        self.assertNotIn('artifact("searchkerneld-", ".tar.gz")', gate)

    def test_packaged_daemon_primary_probe_uses_multi_index_registry(self):
        gate = (ROOT / "tools/qualify-consumer-release.py").read_text()
        self.assertIn('"--data-root"', gate)
        self.assertNotIn('"--index"', gate)
        self.assertNotIn("def start_daemon(", gate)
        self.assertIn("multi_index_registry_probe", gate)
        self.assertIn('target: "release-a"', gate)
        self.assertIn('target: "release-b"', gate)
        self.assertIn('SearchKernel::Index.create(endpoint, fields: fields, target: "release-a", timeout: 120.0)', gate)
        self.assertIn('SearchKernel::Index.open(endpoint, target: "release-a", timeout: 120.0)', gate)
        self.assertIn('x=Index.connect({endpoint!r}, target={target!r})', gate)
        self.assertIn("closing A disrupted B", gate)
        self.assertIn("did not persist across daemon restart", gate)
        lifecycle = gate.split("def with_restarted_daemon", 1)[1].split("\ndef ", 1)[0]
        self.assertIn("start_multi_daemon(daemon, data_root", lifecycle)
        self.assertIn("create_index_target(gem_home, target, http, root)", lifecycle)
        self.assertNotIn("mkdir", lifecycle)

    def test_runtime_negative_probe_breaks_real_packaged_daemon_dependency(self):
        gate = (ROOT / "tools/qualify-consumer-release.py").read_text()
        negative = gate.split("def packaged_daemon_negative_probe", 1)[1].split("\ndef ", 1)[0]
        self.assertIn("packaged_root: Path", negative)
        self.assertIn("shutil.copytree(packaged_root, broken_root, symlinks=True)", negative)
        self.assertIn('"libsearchkernel_vespa.so.2"', negative)
        self.assertIn("adapter.unlink()", negative)
        self.assertIn('"--data-root"', negative)
        self.assertIn('"--listen"', negative)
        self.assertNotIn("--searchkernel-runtime-probe", negative)
        self.assertIn("socket.create_connection", negative)
        self.assertIn("clean_environment()", negative)
        self.assertIn('packaged_daemon_negative_probe(root, context["daemon"].parent.parent)', gate)
        self.assertNotIn('"--index"', negative)
        self.assertNotIn("SEARCHKERNEL_LOCAL_HOST", negative)

    def test_public_quickstarts_are_canonical_and_rendered(self):
        quickstarts = ROOT / "examples" / "release-quickstart"
        required = [
            quickstarts / "python.py",
            quickstarts / "typescript.mjs",
            quickstarts / "ruby.rb",
            quickstarts / "rust" / "Cargo.toml",
            quickstarts / "rust" / "src" / "main.rs",
        ]
        for path in required:
            self.assertTrue(path.is_file(), f"missing canonical release quickstart: {path}")

        gate = (ROOT / "tools/qualify-consumer-release.py").read_text()
        self.assertIn('QUICKSTARTS = QUALIFICATION_ROOT / "quickstarts"', gate)
        self.assertIn("run_python_quickstart", gate)
        self.assertIn("run_node_quickstart", gate)
        self.assertIn("run_ruby_quickstart", gate)
        self.assertIn("run_rust_quickstart", gate)
        for canonical_path in (
            'QUICKSTARTS / "python.py"',
            'QUICKSTARTS / "typescript.mjs"',
            'QUICKSTARTS / "ruby.rb"',
            'QUICKSTARTS / "rust"',
        ):
            self.assertIn(canonical_path, gate)
        docs = (ROOT / "docs" / "release-support.md").read_text(encoding="utf-8")
        self.assertRegex(docs, r"\[[^\]\n]+\]\(release-quickstart\.md\)")
        self.assertNotIn("```", docs, "release support docs must not duplicate executable examples")

        renderer = ROOT / "tools" / "render-release-quickstarts.py"
        result = subprocess.run(
            [sys.executable, str(renderer), "--check"],
            cwd=ROOT,
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )
        self.assertEqual(
            result.returncode,
            0,
            f"canonical release quickstart docs are stale:\n{result.stdout}{result.stderr}",
        )

        output = ROOT / "docs" / "release-quickstart.md"
        original = output.read_text(encoding="utf-8")
        try:
            output.write_text(original + "\n<!-- stale -->\n", encoding="utf-8")
            stale = subprocess.run(
                [sys.executable, str(renderer), "--check"],
                cwd=ROOT,
                text=True,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
            )
            self.assertNotEqual(stale.returncode, 0, "--check accepted stale generated docs")
        finally:
            output.write_text(original, encoding="utf-8")

    def test_release_packager_builds_the_real_ruby_gem(self):
        source = ast.parse((ROOT / "tools/package-consumer-release.py").read_text())
        gem_builds = [
            node for node in ast.walk(source)
            if isinstance(node, ast.Call)
            and isinstance(node.func, ast.Name)
            and node.func.id == "run"
            and len(node.args) >= 2
            and all(isinstance(arg, ast.Constant) for arg in node.args[:2])
            and [arg.value for arg in node.args[:2]] == ["gem", "build"]
        ]
        self.assertEqual(len(gem_builds), 1)
        gate = (ROOT / "tools/qualify-consumer-release.py").read_text()
        self.assertIn('run("gem", "install", "--local", "--install-dir"', gate)
        self.assertIn('"ruby/connect": "ruby_connect"', gate)


if __name__ == "__main__":
    unittest.main()
