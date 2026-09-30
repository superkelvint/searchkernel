#!/usr/bin/env python3
import json
import ast
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


class QualificationPolicyTests(unittest.TestCase):
    def test_gate_runs_in_consumer_container_without_producer_state(self):
        runner = (ROOT / ".github/ci/ci-image-runner.yml").read_text()
        marker = 'echo "==> Qualify packaged release in clean consumer"'
        self.assertIn(marker, runner)
        section = runner.split(marker, 1)[1].split("\n          fi", 1)[0]
        self.assertIn('-v "$ARTIFACT_DIR/consumer-release:/release:ro"', section)
        self.assertIn('-v "$ARTIFACT_DIR/consumer-release-tools:/qualification:ro"', section)
        self.assertIn("python3 /qualification/qualify-consumer-release.py /release", section)
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
            "cp tools/qualify-consumer-release.py /artifacts/consumer-release-tools/",
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
