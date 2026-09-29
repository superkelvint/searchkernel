#!/usr/bin/env python3
import importlib.util
import json
import pathlib
import subprocess
import tempfile
import unittest

ROOT = pathlib.Path(__file__).resolve().parents[1]
REGISTRY = ROOT / "build" / "targets.json"
TOOL = ROOT / "tools" / "dev-targets.py"

class DevTargetsTest(unittest.TestCase):
    def test_registry_loader_rejects_duplicate_keys(self):
        spec = importlib.util.spec_from_file_location("dev_targets", TOOL)
        module = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(module)
        with tempfile.TemporaryDirectory() as directory:
            registry = pathlib.Path(directory) / "targets.json"
            registry.write_text(
                '{"verify":{"vespa.search-reply":{},"vespa.search-reply":{"aliases":["searchreply"]}}}'
            )
            module.REGISTRY = registry

            with self.assertRaisesRegex(ValueError, "duplicate key: vespa.search-reply"):
                module.load()

    def test_registry_has_required_public_commands(self):
        data = json.loads(REGISTRY.read_text())
        self.assertEqual(
            set(data["commands"]),
            {"doctor", "build", "ci", "test", "lint", "verify", "universe", "release-check"},
        )

    def test_aliases_resolve_to_canonical_targets(self):
        out = subprocess.check_output([TOOL, "resolve", "test", "rust"], text=True).strip()
        self.assertEqual(out, "rust-api")
        out = subprocess.check_output([TOOL, "resolve", "verify", "m14"], text=True).strip()
        self.assertEqual(out, "milestone.m14")

    def test_list_contains_every_canonical_target_once(self):
        data = json.loads(REGISTRY.read_text())
        for kind in ("test", "verify"):
            output = subprocess.check_output([TOOL, "list", kind], text=True)
            listed = [line.strip() for line in output.splitlines() if line.strip()]
            self.assertEqual(listed, list(data[kind]))

    def test_aliases_do_not_create_duplicate_discovery_targets(self):
        data = json.loads(REGISTRY.read_text())
        output = subprocess.check_output([TOOL, "list", "verify"], text=True)
        listed = {line.strip() for line in output.splitlines()}
        for meta in data["verify"].values():
            self.assertTrue(listed.isdisjoint(meta.get("aliases", [])))

    def test_client_semantic_parity_is_registered(self):
        data = json.loads(REGISTRY.read_text())
        self.assertIn("client-semantic-parity", data["verify"])
        out = subprocess.check_output(
            [TOOL, "resolve", "verify", "client-semantic-parity"], text=True
        ).strip()
        self.assertEqual(out, "client-semantic-parity")

    def test_client_semantic_parity_has_one_snapshot_authority(self):
        driver = (ROOT / "tools" / "check-client-semantic-parity.py").read_text()
        self.assertIn("CARGO_TARGET_DIR", driver)
        self.assertIn("SEARCHKERNEL_SEMANTIC_SNAPSHOT", driver)
        self.assertNotIn("ROOT/'target/debug/searchkernel-semantic-snapshot'", driver)

        for relative in (
            "clients/python/tests/test_semantic_parity.py",
            "clients/typescript/tests/semantic-parity.test.ts",
            "clients/php/tests/semantic_parity.php",
            "clients/ruby/test/semantic_parity.rb",
        ):
            text = (ROOT / relative).read_text()
            self.assertIn("SEARCHKERNEL_SEMANTIC_SNAPSHOT", text, relative)
            self.assertNotIn("target/debug/searchkernel-semantic-snapshot", text, relative)

        self.assertNotIn("cargo','build", (ROOT / "clients/python/tests/test_semantic_parity.py").read_text())
        self.assertNotIn('execFileSync("cargo"', (ROOT / "clients/typescript/tests/semantic-parity.test.ts").read_text())

    def test_unknown_target_fails_closed(self):
        result = subprocess.run([TOOL, "resolve", "verify", "nope"], text=True, capture_output=True)
        self.assertEqual(result.returncode, 2)
        self.assertIn("unknown verify target: nope", result.stderr)

    def test_change_area_guidance_only_references_registered_targets(self):
        data = json.loads(REGISTRY.read_text())
        for targets in data["change_areas"].values():
            for ref in targets:
                kind, target = ref.split(":", 1)
                self.assertIn(target, data[kind])

if __name__ == "__main__":
    unittest.main()
