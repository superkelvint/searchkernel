#!/usr/bin/env python3
import importlib.util
import json
import pathlib
import re
import subprocess
import tempfile
import unittest

ROOT = pathlib.Path(__file__).resolve().parents[1]
REGISTRY = ROOT / "build" / "targets.json"
TOOL = ROOT / "tools" / "dev-targets.py"


def markdown_anchor(heading):
    heading = re.sub(r"<[^>]+>", "", heading.strip().lower())
    heading = re.sub(r"[^\w\- ]", "", heading)
    return re.sub(r"\s+", "-", heading)


class DevTargetsTest(unittest.TestCase):
    def explain(self, path):
        return subprocess.run(
            ["./dev", "explain", path],
            cwd=ROOT,
            text=True,
            capture_output=True,
        )

    def assert_check_reference(self, data, reference):
        kind, target = reference.split(":", 1)
        if kind == "command":
            self.assertIn(target, data["commands"])
            self.assertEqual(data["commands"][target]["status"], "implemented")
            return
        self.assertIn(kind, {"test", "verify"})
        self.assertIn(target, data[kind])

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
        self.assertEqual(data["schema_version"], 2)
        self.assertEqual(
            set(data["commands"]),
            {
                "doctor",
                "build",
                "ci",
                "test",
                "lint",
                "verify",
                "galaxy",
                "universe",
                "release-check",
                "check-fast",
                "check-protocol",
                "explain",
                "bookkeeping",
            },
        )

    def test_aliases_resolve_to_canonical_targets(self):
        out = subprocess.check_output([TOOL, "resolve", "test", "rust"], text=True).strip()
        self.assertEqual(out, "rust-api")
        out = subprocess.check_output(
            [TOOL, "resolve", "test", "rust-native-e2e"], text=True
        ).strip()
        self.assertEqual(out, "rust-native-e2e")
        out = subprocess.check_output([TOOL, "resolve", "verify", "m14"], text=True).strip()
        self.assertEqual(out, "milestone.m14")

    def test_list_contains_every_canonical_target_once(self):
        data = json.loads(REGISTRY.read_text())
        for kind in ("test", "verify"):
            output = subprocess.check_output([TOOL, "list", kind], text=True)
            listed = [line.strip() for line in output.splitlines() if line.strip()]
            self.assertEqual(listed, list(data[kind]))

    def test_universe_list_contains_only_registered_semantic_verifiers(self):
        data = json.loads(REGISTRY.read_text())
        expected = [
            name
            for name, meta in data["verify"].items()
            if meta.get("universe") == "semantic"
        ]
        output = subprocess.check_output([TOOL, "list", "universe"], text=True)
        self.assertEqual(output.splitlines(), expected)
        self.assertTrue(expected)
        self.assertTrue(all(name.startswith(("vespa.", "product.")) for name in expected))

    def test_aliases_do_not_create_duplicate_discovery_targets(self):
        data = json.loads(REGISTRY.read_text())
        output = subprocess.check_output([TOOL, "list", "verify"], text=True)
        listed = {line.strip() for line in output.splitlines()}
        for meta in data["verify"].values():
            self.assertTrue(listed.isdisjoint(meta.get("aliases", [])))

    def test_contract_verifier_includes_audit_integrity_checks(self):
        driver = (ROOT / "dev").read_text()
        contracts = driver.split("        contracts)\n", 1)[1].split("            ;;\n", 1)[0]
        bookkeeping = driver.split("bookkeeping_command() {", 1)[1].split("check_protocol() {", 1)[0]
        for script in ("check-audit-split.py", "check-audit-client-coverage.py"):
            self.assertIn(script, contracts)
            self.assertIn(script, bookkeeping)


    def test_consumer_release_is_registered_and_dispatched(self):
        data = json.loads(REGISTRY.read_text())
        self.assertIn("consumer-release", data["test"])
        out = subprocess.check_output(
            [TOOL, "resolve", "test", "consumer-release"], text=True
        ).strip()
        self.assertEqual(out, "consumer-release")

        driver = (ROOT / "dev").read_text()
        case = driver.split("        consumer-release)\n", 1)[1].split(
            "            ;;\n", 1
        )[0]
        self.assertIn("tools/package-consumer-release.py", case)
        self.assertIn("tools/qualify-consumer-release.py", case)

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
        self.assertNotIn(
            "cargo','build",
            (ROOT / "clients/python/tests/test_semantic_parity.py").read_text(),
        )
        self.assertNotIn(
            'execFileSync("cargo"',
            (ROOT / "clients/typescript/tests/semantic-parity.test.ts").read_text(),
        )

    def test_result_schema_changes_select_client_semantic_parity(self):
        for path in (
            "api/searchkernel.fbs",
            "core/crates/searchkernel-protocol/src/lib.rs",
            "core/crates/searchkernel-protocol/src/generated/search_kernel/wire/search_result_generated.rs",
            "core/crates/searchkernel-protocol/bfbs/searchkernel_response.bfbs",
            "reference/result-response-conformance/coverage.json",
            "tools/check-flatbuffers-bfbs.sh",
            "tools/check-result-response-schema.py",
            "tools/test_check_result_response_schema.py",
            "tools/flatbuffers/reflection.fbs",
            "dev",
        ):
            with self.subTest(path=path):
                areas = json.loads(REGISTRY.read_text())["change_areas"]
                matching = [
                    (prefix, metadata)
                    for prefix, metadata in areas.items()
                    if path == prefix.rstrip("/")
                    or (prefix.endswith("/") and path.startswith(prefix))
                ]
                self.assertTrue(matching, path)
                _, metadata = max(matching, key=lambda item: len(item[0]))
                self.assertIn("verify:client-semantic-parity", metadata["verify"])

    def test_parity_verifies_bfbs_freshness_before_result_schema_walker(self):
        driver = (ROOT / "dev").read_text()
        parity = driver.split("client-semantic-parity)", 1)[1].split("abi)", 1)[0]
        self.assertLess(
            parity.index('bash "$ROOT/tools/check-flatbuffers-bfbs.sh"'),
            parity.index('"$PYTHON" "$ROOT/tools/check-result-response-schema.py"'),
        )

    def test_unknown_target_fails_closed(self):
        result = subprocess.run(
            [TOOL, "resolve", "verify", "nope"], text=True, capture_output=True
        )
        self.assertEqual(result.returncode, 2)
        self.assertIn("unknown verify target: nope", result.stderr)

        universe_resolve = subprocess.run(
            [TOOL, "resolve", "universe", "vespa.ranking"],
            text=True,
            capture_output=True,
        )
        self.assertEqual(universe_resolve.returncode, 1)
        self.assertIn(
            "universe is a list-only verifier collection", universe_resolve.stderr
        )

    def test_change_area_guidance_is_self_consistent(self):
        data = json.loads(REGISTRY.read_text())
        required = {
            "owner",
            "area",
            "read",
            "architecture",
            "test",
            "verify",
            "focused_check",
            "strong_check",
            "native_base_rebuild",
            "generated",
            "rules",
        }
        for prefix, metadata in data["change_areas"].items():
            with self.subTest(prefix=prefix):
                registered_path = ROOT / prefix.rstrip("/")
                self.assertTrue(registered_path.exists(), prefix)
                self.assertTrue(required.issubset(metadata))
                self.assertNotIn("native_sdk_rebuild", metadata)

                for kind in ("test", "verify"):
                    for reference in metadata[kind]:
                        target_kind, target = reference.split(":", 1)
                        self.assertEqual(target_kind, kind)
                        self.assertIn(target, data[kind])

                self.assert_check_reference(data, metadata["focused_check"])
                self.assert_check_reference(data, metadata["strong_check"])

                for relative in metadata["read"]:
                    path = pathlib.PurePosixPath(relative)
                    self.assertFalse(path.is_absolute())
                    self.assertNotIn("..", path.parts)
                    self.assertTrue((ROOT / path).is_file(), relative)

                architecture_path, separator, anchor = metadata[
                    "architecture"
                ].partition("#")
                self.assertTrue((ROOT / architecture_path).is_file(), architecture_path)
                if separator:
                    anchors = set()
                    for line in (ROOT / architecture_path).read_text().splitlines():
                        match = re.match(r"^#{1,6}\s+(.+?)\s*#*\s*$", line)
                        if match:
                            anchors.add(markdown_anchor(match.group(1)))
                    self.assertIn(anchor, anchors, metadata["architecture"])

    def test_explain_uses_explicit_checks_not_array_order(self):
        result = self.explain("core/crates/searchkernel-dispatch/src/lib.rs")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("Focused check: ./dev test rust-api", result.stdout)
        self.assertIn("Strong check: ./dev verify product.integration", result.stdout)

    def test_build_and_tooling_changes_run_the_registry_guard(self):
        for path in ("build/targets.json", "tools/dev-targets.py", "dev"):
            with self.subTest(path=path):
                result = self.explain(path)
                self.assertEqual(result.returncode, 0, result.stderr)
                self.assertIn("Focused check: ./dev check-fast", result.stdout)
                self.assertIn("Strong check: ./dev check-fast", result.stdout)

    def test_generated_rpc_stubs_are_never_reported_as_handwritten(self):
        result = self.explain(
            "clients/python/searchkernel/rpc/searchkernel_pb2.py"
        )
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("Area: Generated Python protobuf bindings", result.stdout)
        self.assertIn("Generated file: yes", result.stdout)
        self.assertIn("Focused check: ./dev check-protocol", result.stdout)
        self.assertIn("Strong check: ./dev test python", result.stdout)

    def test_native_base_rebuild_guidance_is_path_specific(self):
        adapter = self.explain("core/native/vespa/abi.cc")
        self.assertEqual(adapter.returncode, 0, adapter.stderr)
        self.assertIn("Native base rebuild: no", adapter.stdout)

        manifest = self.explain("core/native/vespa/manifest.json")
        self.assertEqual(manifest.returncode, 0, manifest.stderr)
        self.assertIn("Area: Pinned Vespa native base manifest", manifest.stdout)
        self.assertIn("Native base rebuild: yes", manifest.stdout)

    def test_explain_covers_control_plane_and_unmapped_core_crate_paths(self):
        for path, area in (
            ("core/crates/flatbuffers-build/LICENSE", "Rust crate files"),
            ("core/crates/searchkernel-filter/src/lib.rs", "Filter syntax parser"),
            ("core/crates/searchkernel-rpc/src/lib.rs", "Rust gRPC client"),
            (".github/workflows/rust-ci.yml", "GitHub Actions and CI control plane"),
            ("docker/searchkernel-ci-toolchain/Dockerfile", "Canonical CI image definitions"),
        ):
            with self.subTest(path=path):
                result = self.explain(path)
                self.assertEqual(result.returncode, 0, result.stderr)
                self.assertIn("Owner:", result.stdout)
                self.assertIn(f"Area: {area}", result.stdout)

    def test_exact_file_areas_do_not_match_similarly_prefixed_paths(self):
        result = self.explain("developer-notes")
        self.assertEqual(result.returncode, 2)
        self.assertIn("no change area registered", result.stderr)


if __name__ == "__main__":
    unittest.main()
