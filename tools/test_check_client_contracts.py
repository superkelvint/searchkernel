#!/usr/bin/env python3
"""Tests for executable client capability evidence."""

from __future__ import annotations

import importlib.util
import json
import os
import subprocess
import tempfile
import unittest
from unittest import mock
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
MODULE_PATH = ROOT / "tools" / "check-client-contracts.py"
SPEC = importlib.util.spec_from_file_location("check_client_contracts", MODULE_PATH)
assert SPEC and SPEC.loader
MODULE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MODULE)

CATALOG = {
    "version": 1,
    "surfaces": {
        "rust": {
            "title": "Rust",
            "runtimes": ["native"],
            "evidence_roots": ["rust-tests"],
            "e2e_lanes": [
                {
                    "runtimes": ["native"],
                    "path": "rust-tests/e2e.rs",
                    "command": ["python3", "-c", "pass"],
                }
            ],
            "required_e2e_runtimes": ["native"],
        },
        "python": {
            "title": "Python",
            "runtimes": ["grpc"],
            "evidence_roots": ["python-tests"],
            "e2e_lanes": [
                {
                    "runtimes": ["grpc"],
                    "path": "python-tests/e2e.py",
                    "command": ["python3", "-c", "pass"],
                }
            ],
        },
        "ruby": {
            "title": "Ruby",
            "runtimes": ["http", "managed-local"],
            "evidence_roots": ["ruby-tests"],
            "e2e_lanes": [
                {
                    "runtimes": ["managed-local"],
                    "path": "ruby-tests/e2e.rb",
                    "command": ["ruby", "ruby-tests/e2e.rb"],
                }
            ],
        },
    },
    "parity_requirements": [
        {
            "source_surface": "python",
            "target_surface": "ruby",
            "minimum_strength": "runtime",
            "required_kinds": ["ergonomic", "e2e"],
            "required_contracts": ["query.phrase/basic"],
            "source_runtimes": ["grpc"],
            "target_runtimes": ["managed-local"],
        }
    ],
    "scenarios": [
        {
            "capability": "result.highlighting",
            "id": "basic",
            "title": "Basic highlights",
        },
        {
            "capability": "result.highlighting",
            "id": "literal-html",
            "title": "Literal HTML",
        },
    ],
}


class ClientContractTests(unittest.TestCase):
    def setUp(self) -> None:
        self.tempdir = tempfile.TemporaryDirectory()
        self.root = Path(self.tempdir.name)
        (self.root / "docs").mkdir()
        (self.root / "reference" / "client-contracts").mkdir(parents=True)
        (self.root / "rust-tests").mkdir()
        (self.root / "python-tests").mkdir()
        (self.root / "ruby-tests").mkdir()
        (self.root / "rust-tests" / "e2e.rs").write_text("", encoding="utf-8")
        (self.root / "python-tests" / "e2e.py").write_text("", encoding="utf-8")
        (self.root / "ruby-tests" / "e2e.rb").write_text("", encoding="utf-8")
        (self.root / "docs" / "capabilities.json").write_text(
            json.dumps(
                {
                    "version": 1,
                    "capabilities": [
                        {
                            "id": "result.highlighting",
                            "title": "Highlighting",
                            "area": "result",
                        },
                        {
                            "id": "search.pagination",
                            "title": "Pagination",
                            "area": "search",
                        },
                        {
                            "id": "query.phrase",
                            "title": "Phrase queries",
                            "area": "query",
                        },
                    ],
                }
            ),
            encoding="utf-8",
        )
        (
            self.root / "reference" / "client-contracts" / "catalog.json"
        ).write_text(json.dumps(CATALOG), encoding="utf-8")

    def tearDown(self) -> None:
        self.tempdir.cleanup()

    def load(self):
        global_caps = MODULE.load_global_capabilities(
            self.root / "docs" / "capabilities.json"
        )
        catalog = MODULE.load_catalog(
            self.root / "reference" / "client-contracts" / "catalog.json"
        )
        surfaces, scenarios, overrides = MODULE.validate_catalog(
            catalog, global_caps, self.root
        )
        return global_caps, surfaces, scenarios, overrides

    def test_committed_catalog_pins_python_ruby_runtime_facade_parity(self) -> None:
        catalog = MODULE.load_catalog()
        expected = {
            "source_surface": "python",
            "target_surface": "ruby",
            "minimum_strength": "runtime",
            "required_kinds": ["ergonomic", "e2e"],
            "required_contracts": [
                "update.document_crud/basic",
                "query.phrase/basic",
                "query.proximity/basic",
                "query.fuzzy/basic",
                "query.weighted_and_wand/basic",
                "query.same_element/basic",
                "query.geo/basic",
                "result.docsum_field_projection/basic",
                "result.highlighting/basic",
            ],
            "source_runtimes": ["grpc", "managed-local"],
            "target_runtimes": ["managed-local"],
        }
        self.assertIn(expected, catalog.get("parity_requirements", []))

    def test_committed_catalog_runs_python_public_facade_e2e(self) -> None:
        catalog = MODULE.load_catalog()
        self.assertIn(
            {
                "runtimes": ["managed-local"],
                "path": "clients/python/examples/try_searchkernel_client_api_e2e.py",
                "command": [
                    "bash",
                    "tools/run-python-native-e2e.sh",
                    "--include-facade",
                ],
            },
            catalog["surfaces"]["python"]["e2e_lanes"],
        )

    def test_committed_registry_and_reports_are_valid(self) -> None:
        report = MODULE.build_current_report()
        self.assertEqual(
            MODULE.check_documents(MODULE.generated_documents(report)),
            [],
        )

    def _write_fake_lifecycle_bfbs(self) -> tuple[Path, Path]:
        bfbs = (
            self.root
            / "core"
            / "crates"
            / "searchkernel-protocol"
            / "bfbs"
        )
        bfbs.mkdir(parents=True, exist_ok=True)
        request = bfbs / "searchkernel_request.bfbs"
        response = bfbs / "searchkernel_response.bfbs"
        request.write_bytes(b"request")
        response.write_bytes(b"response")
        return request, response

    @staticmethod
    def _lifecycle_payloads(operations, suffix):
        return tuple(
            f"SearchKernel.Wire.{operation}{suffix}"
            for operation in operations
        )

    def _minimal_lifecycle_catalog(
        self,
        surface_name: str,
        public_path: str,
        *,
        operations=("CreateIndex", "OpenIndex", "DeleteIndex"),
    ) -> dict:
        wire_path = f"{surface_name}-wire.txt"
        (self.root / wire_path).write_text(
            "\n".join(
                f"{operation}Request {operation}Result"
                for operation in operations
            ),
            encoding="utf-8",
        )
        return {
            "lifecycle_surfaces": [surface_name],
            "surfaces": {
                surface_name: {
                    "lifecycle_contract": {
                        operation: [
                            {
                                "kind": "public",
                                "path": public_path,
                                "contains": f"public:{operation}",
                            },
                            {
                                "kind": "wire",
                                "path": wire_path,
                                "contains": f"{operation}Request {operation}Result",
                            },
                        ]
                        for operation in operations
                    }
                }
            },
        }

    def test_lifecycle_parity_fails_closed_without_canonical_bfbs(self) -> None:
        with self.assertRaisesRegex(
            MODULE.RegistryError,
            "canonical BFBS lifecycle metadata is missing",
        ):
            MODULE.canonical_lifecycle_operations(self.root)

    def test_bfbs_lifecycle_parity_tracks_future_operations(self) -> None:
        request, _response = self._write_fake_lifecycle_bfbs()
        operations = ("CreateIndex", "OpenIndex", "DeleteIndex", "RenameIndex")

        def reflected(path: Path, _union_name: str) -> tuple[str, ...]:
            suffix = "Request" if path == request else "Result"
            return self._lifecycle_payloads(operations, suffix)

        with mock.patch.object(
            MODULE,
            "_reflected_union_payloads",
            side_effect=reflected,
        ):
            self.assertEqual(
                MODULE.canonical_lifecycle_operations(self.root),
                operations,
            )

    def test_lifecycle_parity_rejects_request_result_union_drift(self) -> None:
        request, _response = self._write_fake_lifecycle_bfbs()

        def reflected(path: Path, _union_name: str) -> tuple[str, ...]:
            operations = (
                ("CreateIndex", "OpenIndex", "DeleteIndex", "RenameIndex")
                if path == request
                else ("CreateIndex", "OpenIndex", "DeleteIndex")
            )
            suffix = "Request" if path == request else "Result"
            return self._lifecycle_payloads(operations, suffix)

        with mock.patch.object(
            MODULE,
            "_reflected_union_payloads",
            side_effect=reflected,
        ):
            with self.assertRaisesRegex(
                MODULE.RegistryError,
                "missing result\\(s\\): RenameIndex",
            ):
                MODULE.canonical_lifecycle_operations(self.root)

    def test_lifecycle_parity_requires_public_api_not_just_wire_tokens(self) -> None:
        request, _response = self._write_fake_lifecycle_bfbs()
        operations = ("CreateIndex", "OpenIndex", "DeleteIndex")

        def reflected(path: Path, _union_name: str) -> tuple[str, ...]:
            suffix = "Request" if path == request else "Result"
            return self._lifecycle_payloads(operations, suffix)

        for surface_name in ("python", "ruby"):
            with self.subTest(surface=surface_name):
                public_path = f"{surface_name}-public.txt"
                (self.root / public_path).write_text(
                    "public:CreateIndex\npublic:OpenIndex\n",
                    encoding="utf-8",
                )
                catalog = self._minimal_lifecycle_catalog(
                    surface_name,
                    public_path,
                )
                with mock.patch.object(
                    MODULE,
                    "_reflected_union_payloads",
                    side_effect=reflected,
                ):
                    with self.assertRaisesRegex(
                        MODULE.RegistryError,
                        rf"surface {surface_name} lifecycle DeleteIndex.*"
                        "missing required source evidence",
                    ):
                        MODULE.validate_client_lifecycle_parity(
                            catalog,
                            self.root,
                        )

    def test_future_lifecycle_operation_requires_every_surface_contract_to_follow(self) -> None:
        request, _response = self._write_fake_lifecycle_bfbs()
        canonical_operations = (
            "CreateIndex",
            "OpenIndex",
            "DeleteIndex",
            "RenameIndex",
        )

        def reflected(path: Path, _union_name: str) -> tuple[str, ...]:
            suffix = "Request" if path == request else "Result"
            return self._lifecycle_payloads(canonical_operations, suffix)

        public_path = "python-public.txt"
        (self.root / public_path).write_text(
            "public:CreateIndex\npublic:OpenIndex\npublic:DeleteIndex\n",
            encoding="utf-8",
        )
        catalog = self._minimal_lifecycle_catalog("python", public_path)
        with mock.patch.object(
            MODULE,
            "_reflected_union_payloads",
            side_effect=reflected,
        ):
            with self.assertRaisesRegex(
                MODULE.RegistryError,
                "missing operation\\(s\\): RenameIndex",
            ):
                MODULE.validate_client_lifecycle_parity(catalog, self.root)

    def test_evidence_is_discovered_and_rendered(self) -> None:
        (self.root / "rust-tests" / "e2e.rs").write_text(
            "// SKC: result.highlighting/basic "
            "kinds=ergonomic,e2e runtime=native\n",
            encoding="utf-8",
        )
        global_caps, surfaces, scenarios, overrides = self.load()
        evidence = MODULE.collect_evidence(surfaces, scenarios, self.root)
        self.assertEqual({item["kind"] for item in evidence}, {"ergonomic", "e2e"})
        report = MODULE.build_report(
            global_caps, surfaces, scenarios, evidence, overrides
        )
        markdown = MODULE.render_markdown(report)
        self.assertIn("ergonomic · real-runtime 1/2 (native)", markdown)
        self.assertIn("rust-tests/e2e.rs", markdown)

    def test_registered_e2e_lane_does_not_imply_runtime_without_e2e_kind(self) -> None:
        (self.root / "python-tests" / "e2e.py").write_text(
            "# SKC: search.pagination/basic kinds=ergonomic runtime=grpc\n",
            encoding="utf-8",
        )
        _, surfaces, scenarios, _ = self.load()
        evidence = MODULE.collect_evidence(surfaces, scenarios, self.root)
        item = next(entry for entry in evidence if entry["surface"] == "python")
        self.assertEqual(item["kind"], "ergonomic")
        self.assertEqual(item["strength"], "builder")

    def test_explicit_e2e_in_registered_lane_marks_only_e2e_runtime_strength(self) -> None:
        (self.root / "python-tests" / "e2e.py").write_text(
            "# SKC: search.pagination/basic kinds=ergonomic,e2e runtime=grpc\n",
            encoding="utf-8",
        )
        _, surfaces, scenarios, _ = self.load()
        evidence = MODULE.collect_evidence(surfaces, scenarios, self.root)
        strengths = {
            item["kind"]: item["strength"]
            for item in evidence
            if item["surface"] == "python"
        }
        self.assertEqual(strengths, {"ergonomic": "builder", "e2e": "runtime"})

    def test_runtime_parity_rejects_missing_required_source_contract(self) -> None:
        _, surfaces, scenarios, overrides = self.load()
        evidence = MODULE.collect_evidence(surfaces, scenarios, self.root)
        with self.assertRaisesRegex(
            MODULE.RegistryError,
            "required source parity evidence missing.*query.phrase/basic",
        ):
            MODULE.validate_parity_evidence(CATALOG, surfaces, evidence, overrides)

    def test_runtime_parity_rejects_builder_only_target_evidence(self) -> None:
        (self.root / "python-tests" / "e2e.py").write_text(
            "# SKC: query.phrase/basic kinds=ergonomic,e2e runtime=grpc\n",
            encoding="utf-8",
        )
        (self.root / "ruby-tests" / "builder.rb").write_text(
            "# SKC: query.phrase/basic kinds=ergonomic runtime=http\n",
            encoding="utf-8",
        )
        _, surfaces, scenarios, overrides = self.load()
        evidence = MODULE.collect_evidence(surfaces, scenarios, self.root)
        with self.assertRaisesRegex(
            MODULE.RegistryError,
            "runtime parity mismatch.*query.phrase/basic.*python.*ruby",
        ):
            MODULE.validate_parity_evidence(CATALOG, surfaces, evidence, overrides)

    def test_runtime_name_without_registered_lane_remains_builder_strength(self) -> None:
        (self.root / "ruby-tests" / "builder.rb").write_text(
            "# SKC: search.pagination/basic kinds=ergonomic runtime=managed-local\n",
            encoding="utf-8",
        )
        _, surfaces, scenarios, _ = self.load()
        evidence = MODULE.collect_evidence(surfaces, scenarios, self.root)
        item = next(entry for entry in evidence if entry["surface"] == "ruby")
        self.assertEqual(item["runtime"], "managed-local")
        self.assertEqual(item["strength"], "builder")

    def test_runtime_parity_rejects_split_builder_and_low_level_e2e_evidence(self) -> None:
        (self.root / "python-tests" / "e2e.py").write_text(
            "# SKC: query.phrase/basic kinds=ergonomic,e2e runtime=grpc\n",
            encoding="utf-8",
        )
        (self.root / "ruby-tests" / "builder.rb").write_text(
            "# SKC: query.phrase/basic kinds=ergonomic runtime=http\n",
            encoding="utf-8",
        )
        (self.root / "ruby-tests" / "e2e.rb").write_text(
            "# SKC: query.phrase/basic kinds=e2e runtime=managed-local\n",
            encoding="utf-8",
        )
        _, surfaces, scenarios, overrides = self.load()
        evidence = MODULE.collect_evidence(surfaces, scenarios, self.root)
        with self.assertRaisesRegex(
            MODULE.RegistryError,
            "runtime parity mismatch.*query.phrase/basic.*python.*ruby",
        ):
            MODULE.validate_parity_evidence(CATALOG, surfaces, evidence, overrides)

    def test_runtime_parity_allows_explicit_blocked_override(self) -> None:
        (self.root / "python-tests" / "e2e.py").write_text(
            "# SKC: query.phrase/basic kinds=ergonomic,e2e runtime=grpc\n",
            encoding="utf-8",
        )
        (self.root / "ruby-tests" / "builder.rb").write_text(
            "# SKC: query.phrase/basic kinds=ergonomic runtime=http\n",
            encoding="utf-8",
        )
        _, surfaces, scenarios, _ = self.load()
        evidence = MODULE.collect_evidence(surfaces, scenarios, self.root)
        overrides = [
            {
                "capability": "query.phrase",
                "scenario": "basic",
                "surface": "ruby",
                "runtime": "managed-local",
                "status": "blocked",
                "reason": "real runtime evidence pending",
                "issue": 803,
            }
        ]
        MODULE.validate_parity_evidence(CATALOG, surfaces, evidence, overrides)

    def test_runtime_parity_rejects_stale_override_after_runtime_evidence_lands(self) -> None:
        (self.root / "python-tests" / "e2e.py").write_text(
            "# SKC: query.phrase/basic kinds=ergonomic,e2e runtime=grpc\n",
            encoding="utf-8",
        )
        (self.root / "ruby-tests" / "e2e.rb").write_text(
            "# SKC: query.phrase/basic kinds=ergonomic,e2e runtime=managed-local\n",
            encoding="utf-8",
        )
        _, surfaces, scenarios, _ = self.load()
        evidence = MODULE.collect_evidence(surfaces, scenarios, self.root)
        overrides = [
            {
                "capability": "query.phrase",
                "scenario": "basic",
                "surface": "ruby",
                "runtime": "managed-local",
                "status": "blocked",
                "reason": "real runtime evidence pending",
                "issue": 803,
            }
        ]
        with self.assertRaisesRegex(MODULE.RegistryError, "stale parity override"):
            MODULE.validate_parity_evidence(CATALOG, surfaces, evidence, overrides)

    def test_ruby_evidence_files_are_discovered(self) -> None:
        (self.root / "ruby-tests" / "evidence.rb").write_text(
            "# SKC: search.pagination/basic kinds=ergonomic runtime=managed-local\n",
            encoding="utf-8",
        )
        _, surfaces, scenarios, _ = self.load()
        evidence = MODULE.collect_evidence(surfaces, scenarios, self.root)
        ruby = [item for item in evidence if item["surface"] == "ruby"]
        self.assertEqual(
            ruby,
            [
                {
                    "capability": "search.pagination",
                    "scenario": "basic",
                    "surface": "ruby",
                    "runtime": "managed-local",
                    "kind": "ergonomic",
                    "strength": "builder",
                    "path": "ruby-tests/evidence.rb",
                    "line": 1,
                }
            ],
        )

    def test_run_e2e_lanes_can_select_one_surface(self) -> None:
        _, surfaces, _, _ = self.load()
        with mock.patch.object(MODULE.subprocess, "run") as run:
            MODULE.run_e2e_lanes(surfaces, self.root, surface="rust")
        run.assert_called_once_with(
            ("python3", "-c", "pass"),
            cwd=self.root,
            check=True,
        )

    def test_run_e2e_lanes_rejects_unknown_surface(self) -> None:
        _, surfaces, _, _ = self.load()
        with self.assertRaisesRegex(MODULE.RegistryError, "unknown E2E client surface"):
            MODULE.run_e2e_lanes(surfaces, self.root, surface="missing")


    def test_parity_requirement_rejects_unknown_required_kind(self) -> None:
        path = self.root / "reference" / "client-contracts" / "catalog.json"
        catalog = json.loads(json.dumps(CATALOG))
        catalog["parity_requirements"][0]["required_kinds"] = ["ergonomic", "magic"]
        path.write_text(json.dumps(catalog), encoding="utf-8")
        global_caps = MODULE.load_global_capabilities(
            self.root / "docs" / "capabilities.json"
        )
        surfaces, scenarios, overrides = MODULE.validate_catalog(
            MODULE.load_catalog(path), global_caps, self.root
        )
        evidence = MODULE.collect_evidence(surfaces, scenarios, self.root)
        with self.assertRaisesRegex(MODULE.RegistryError, "required_kinds"):
            MODULE.validate_parity_evidence(catalog, surfaces, evidence, overrides)

    def test_required_e2e_runtime_requires_lane(self) -> None:
        path = self.root / "reference" / "client-contracts" / "catalog.json"
        catalog = json.loads(json.dumps(CATALOG))
        catalog["surfaces"]["rust"].pop("e2e_lanes")
        path.write_text(json.dumps(catalog), encoding="utf-8")
        global_caps = MODULE.load_global_capabilities(
            self.root / "docs" / "capabilities.json"
        )
        with self.assertRaisesRegex(
            MODULE.RegistryError,
            "requires an E2E lane for runtime native",
        ):
            MODULE.validate_catalog(
                MODULE.load_catalog(path), global_caps, self.root
            )

    def test_e2e_evidence_requires_ci_lane(self) -> None:
        path = self.root / "reference" / "client-contracts" / "catalog.json"
        catalog = json.loads(json.dumps(CATALOG))
        catalog["surfaces"]["rust"].pop("e2e_lanes")
        catalog["surfaces"]["rust"].pop("required_e2e_runtimes")
        path.write_text(json.dumps(catalog), encoding="utf-8")
        (self.root / "rust-tests" / "e2e.rs").write_text(
            "// SKC: result.highlighting/basic kinds=e2e runtime=native\n",
            encoding="utf-8",
        )
        _, surfaces, scenarios, _ = self.load()
        with self.assertRaisesRegex(MODULE.RegistryError, "not covered by an E2E CI lane"):
            MODULE.collect_evidence(surfaces, scenarios, self.root)

    def test_default_basic_scenario_exists(self) -> None:
        _, _, scenarios, _ = self.load()
        self.assertEqual(scenarios["search.pagination"], {"basic": "Basic behavior"})

    def test_unknown_capability_is_rejected(self) -> None:
        (self.root / "python-tests" / "bad.py").write_text(
            "# SKC: search.nope/basic kinds=e2e runtime=grpc\n",
            encoding="utf-8",
        )
        _, surfaces, scenarios, _ = self.load()
        with self.assertRaisesRegex(MODULE.RegistryError, "unknown capability"):
            MODULE.collect_evidence(surfaces, scenarios, self.root)

    def test_unknown_scenario_is_rejected(self) -> None:
        (self.root / "python-tests" / "bad.py").write_text(
            "# SKC: result.highlighting/nope kinds=e2e runtime=grpc\n",
            encoding="utf-8",
        )
        _, surfaces, scenarios, _ = self.load()
        with self.assertRaisesRegex(MODULE.RegistryError, "unknown scenario"):
            MODULE.collect_evidence(surfaces, scenarios, self.root)

    def test_unknown_runtime_is_rejected(self) -> None:
        (self.root / "python-tests" / "bad.py").write_text(
            "# SKC: result.highlighting/basic kinds=e2e runtime=browser\n",
            encoding="utf-8",
        )
        _, surfaces, scenarios, _ = self.load()
        with self.assertRaisesRegex(MODULE.RegistryError, "unknown runtime"):
            MODULE.collect_evidence(surfaces, scenarios, self.root)

    def test_facade_support_rejects_missing_claim_with_ergonomic_evidence(self) -> None:
        (self.root / "python-tests" / "support.py").write_text(
            "# SKC: result.highlighting/basic kinds=ergonomic runtime=grpc\n",
            encoding="utf-8",
        )
        global_caps, surfaces, scenarios, _ = self.load()
        global_caps["result.highlighting"]["facade_support"] = {
            "python": {"status": "missing", "evidence": []}
        }
        evidence = MODULE.collect_evidence(surfaces, scenarios, self.root)
        with self.assertRaisesRegex(MODULE.RegistryError, "claims python is missing"):
            MODULE.validate_facade_support(global_caps, surfaces, evidence)

    def test_malformed_annotation_is_rejected(self) -> None:
        (self.root / "rust-tests" / "bad.rs").write_text(
            "// SKC: result.highlighting/basic e2e native\n",
            encoding="utf-8",
        )
        _, surfaces, scenarios, _ = self.load()
        with self.assertRaisesRegex(MODULE.RegistryError, "malformed SKC annotation"):
            MODULE.collect_evidence(surfaces, scenarios, self.root)

    def test_blocked_override_requires_issue(self) -> None:
        path = self.root / "reference" / "client-contracts" / "catalog.json"
        catalog = json.loads(json.dumps(CATALOG))
        catalog["overrides"] = [
            {
                "capability": "result.highlighting",
                "scenario": "basic",
                "surface": "python",
                "runtime": "grpc",
                "status": "blocked",
                "reason": "waiting on transport",
            }
        ]
        path.write_text(json.dumps(catalog), encoding="utf-8")
        global_caps = MODULE.load_global_capabilities(
            self.root / "docs" / "capabilities.json"
        )
        with self.assertRaisesRegex(MODULE.RegistryError, "needs an issue number"):
            MODULE.validate_catalog(
                MODULE.load_catalog(path), global_caps, self.root
            )


    def test_shared_python_e2e_command_runs_once(self) -> None:
        surfaces = {
            "python": {
                "e2e_lanes": [
                    {
                        "command": ("bash", "tools/run-python-native-e2e.sh", "--include-facade"),
                    },
                    {
                        "command": ("bash", "tools/run-python-native-e2e.sh", "--include-facade"),
                    },
                ]
            }
        }
        with mock.patch.object(MODULE.subprocess, "run") as run:
            MODULE.run_e2e_lanes(surfaces, self.root, surface="python")
        run.assert_called_once_with(
            ("bash", "tools/run-python-native-e2e.sh", "--include-facade"),
            cwd=self.root,
            check=True,
        )

    def test_python_native_e2e_include_facade_runs_both_paths(self) -> None:
        fixture = self.root / "python-e2e-fixture"
        repo = fixture / "repo"
        tools_dir = repo / "tools"
        tools_dir.mkdir(parents=True)

        runner = tools_dir / "run-python-native-e2e.sh"
        runner.write_text(
            (ROOT / "tools" / "run-python-native-e2e.sh").read_text(encoding="utf-8"),
            encoding="utf-8",
        )
        runner.chmod(0o755)

        (tools_dir / "client-e2e-runtime.sh").write_text(
            """#!/usr/bin/env bash
prepare_client_e2e_runtime() {
    export SEARCHKERNEL_LOCAL_HOST=/tmp/fake-searchkerneld
    export SEARCHKERNEL_SDK_LIB=/tmp/fake-adapter
}
""",
            encoding="utf-8",
        )
        resolve_python = tools_dir / "resolve-python.sh"
        resolve_python.write_text(
            '#!/usr/bin/env bash\nprintf "%s\\n" "$FAKE_PYTHON"\n',
            encoding="utf-8",
        )
        resolve_python.chmod(0o755)

        fake_python = fixture / "python"
        fake_python.write_text(
            """#!/usr/bin/env bash
set -euo pipefail
[[ -n "${SEARCHKERNEL_LOCAL_HOST:-}" ]] || exit 41
[[ "${PYTHONPATH:-}" == *"$REPO_ROOT/clients/python"* ]] || exit 42
printf '%s\\n' "$*" >>"$PYTHON_LOG"
""",
            encoding="utf-8",
        )
        fake_python.chmod(0o755)

        subprocess.run(["git", "init", "-q"], cwd=repo, check=True)
        env = os.environ.copy()
        env.update(
            {
                "FAKE_PYTHON": str(fake_python),
                "PYTHON_LOG": str(fixture / "python.log"),
                "REPO_ROOT": str(repo),
            }
        )
        completed = subprocess.run(
            ["bash", str(runner), "--include-facade"],
            cwd=repo,
            env=env,
            text=True,
            capture_output=True,
            check=False,
        )
        self.assertEqual(completed.returncode, 0, msg=completed.stdout + completed.stderr)
        self.assertEqual(
            (fixture / "python.log").read_text(encoding="utf-8").splitlines(),
            [
                "-m unittest discover -s clients/python/tests -p test_grpc_e2e.py",
                "clients/python/examples/try_searchkernel_client_api_e2e.py",
            ],
        )

    def test_typescript_real_e2e_installs_package_dependencies(self) -> None:
        source = (
            ROOT
            / "clients"
            / "typescript"
            / "scripts"
            / "run-real-e2e.sh"
        )
        fixture = self.root / "typescript-e2e-fixture"
        repo = fixture / "repo"
        script = repo / "clients" / "typescript" / "scripts" / "run-real-e2e.sh"
        script.parent.mkdir(parents=True)
        script.write_text(source.read_text(encoding="utf-8"), encoding="utf-8")
        runtime_helper = repo / "tools" / "client-e2e-runtime.sh"
        runtime_helper.parent.mkdir(parents=True)
        runtime_helper.write_text(
            (ROOT / "tools" / "client-e2e-runtime.sh").read_text(encoding="utf-8"),
            encoding="utf-8",
        )

        host = repo / "target" / "host" / "debug" / "searchkerneld"
        host.parent.mkdir(parents=True)
        host.write_text("#!/usr/bin/env bash\nexit 0\n", encoding="utf-8")
        host.chmod(0o755)
        adapter_lib = repo / "target" / "host" / "native-adapter"
        adapter_lib.mkdir(parents=True)
        (adapter_lib / "libsearchkernel_vespa.so.2").write_bytes(b"fixture")

        dev = repo / "dev"
        dev.write_text("#!/usr/bin/env bash\nexit 0\n", encoding="utf-8")
        dev.chmod(0o755)

        tools_dir = repo / "tools"
        tools_dir.mkdir(parents=True, exist_ok=True)
        resolve_python = tools_dir / "resolve-python.sh"
        resolve_python.write_text(
            "#!/usr/bin/env bash\ncommand -v python3\n",
            encoding="utf-8",
        )
        resolve_python.chmod(0o755)
        resolve_rust = tools_dir / "resolve-rust.sh"
        resolve_rust.write_text(
            "#!/usr/bin/env bash\ncommand -v \"${1:-cargo}\"\n",
            encoding="utf-8",
        )
        resolve_rust.chmod(0o755)

        adapter_resolver = repo / ".github" / "ci" / "resolve-native-adapter-lib.py"
        adapter_resolver.parent.mkdir(parents=True)
        adapter_resolver.write_text(
            """#!/usr/bin/env python3
from pathlib import Path
import sys

target = Path(sys.argv[2]).resolve() / "fake-adapter"
target.mkdir(parents=True, exist_ok=True)
(target / "libsearchkernel_vespa.so.2").touch()
print(target)
""",
            encoding="utf-8",
        )
        adapter_resolver.chmod(0o755)

        fakebin = fixture / "bin"
        fakebin.mkdir(parents=True)
        cargo = fakebin / "cargo"
        cargo.write_text(
            """#!/usr/bin/env bash
set -euo pipefail
mkdir -p "$REPO_ROOT/target/host/debug"
cat >"$REPO_ROOT/target/host/debug/searchkerneld" <<'EOF'
#!/usr/bin/env bash
exit 0
EOF
chmod +x "$REPO_ROOT/target/host/debug/searchkerneld"
""",
            encoding="utf-8",
        )
        cargo.chmod(0o755)

        npm = fakebin / "npm"
        npm.write_text(
            """#!/usr/bin/env bash
set -euo pipefail
printf '%s\\n' "$*" >>"$NPM_LOG"
if [[ "${1:-}" == "ci" ]]; then
    touch "$NPM_READY"
    exit 0
fi
if [[ "${1:-}" == "run" && "${2:-}" == "example:e2e" ]]; then
    if [[ ! -f "$NPM_READY" ]]; then
        echo "TypeScript E2E dependencies were not installed" >&2
        exit 42
    fi
    [[ -n "${SEARCHKERNEL_SDK_LIB:-}" ]] || { echo "native adapter runtime path was not passed to example E2E" >&2; exit 45; }
    exit 0
fi
exit 0
""",
            encoding="utf-8",
        )
        npm.chmod(0o755)

        node = fakebin / "node"
        node.write_text(
            """#!/usr/bin/env bash
set -euo pipefail
[[ -f "$NPM_READY" ]] || { echo "TypeScript E2E dependencies were not installed" >&2; exit 42; }
[[ -n "${SEARCHKERNEL_LOCAL_HOST:-}" ]] || { echo "managed-local host was not passed to installed-package E2E" >&2; exit 43; }
[[ -n "${SEARCHKERNEL_SDK_LIB:-}" ]] || { echo "native adapter runtime path was not passed to installed-package E2E" >&2; exit 45; }
[[ "$1" == "clients/typescript/scripts/verify-installed-managed-local-e2e.cjs" ]] || exit 44
printf '%s\\n' "installed-package-e2e" >>"$NPM_LOG"
""",
            encoding="utf-8",
        )
        node.chmod(0o755)

        env = os.environ.copy()
        env.pop("CARGO_TARGET_DIR", None)
        env.update(
            {
                "PATH": f"{fakebin}:{env['PATH']}",
                "REPO_ROOT": str(repo),
                "NPM_LOG": str(fixture / "npm.log"),
                "NPM_READY": str(fixture / "npm-ready"),
                "SEARCHKERNEL_LOCAL_HOST": str(host),
                "SEARCHKERNEL_SDK_LIB": str(adapter_lib),
            }
        )
        completed = subprocess.run(
            ["bash", str(script)],
            cwd=repo,
            env=env,
            text=True,
            capture_output=True,
            check=False,
        )
        self.assertEqual(
            completed.returncode,
            0,
            msg=completed.stdout + completed.stderr,
        )
        self.assertEqual(
            (fixture / "npm.log").read_text(encoding="utf-8").splitlines(),
            [
                "ci --prefix clients/typescript",
                "run build --prefix clients/typescript",
                "installed-package-e2e",
                "run example:e2e --prefix clients/typescript",
            ],
        )


if __name__ == "__main__":
    unittest.main()
