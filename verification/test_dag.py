#!/usr/bin/env python3
from __future__ import annotations

import subprocess
import tempfile
import unittest
from pathlib import Path

import dag as verification_dag

ROOT = Path(__file__).resolve().parents[1]
DAG_PATH = ROOT / "verification" / "dag.toml"
VESPA_ROUTER = ROOT / "reference" / "vespa" / "verify-through.sh"


class VerificationDagTests(unittest.TestCase):
    def test_current_repository_is_structurally_valid_and_covered(self) -> None:
        current = verification_dag.load_dag(DAG_PATH)
        verification_dag.validate_repository_coverage(current, ROOT)

    def test_every_current_web_feature_family_has_subsystem_ownership(self) -> None:
        current = verification_dag.load_dag(DAG_PATH)
        features = ROOT / "searchkernel-web" / "src" / "features"
        for child in sorted(path for path in features.iterdir() if path.is_dir()):
            probe = f"searchkernel-web/src/features/{child.name}/__coverage_probe__.tsx"
            with self.subTest(feature=child.name):
                subsystems = verification_dag.matching_subsystems(current, probe)
                self.assertTrue(
                    any(subsystem.behavioral for subsystem in subsystems),
                    f"{child.name} has no behavioral DAG owner",
                )

    def test_new_core_crate_is_not_silently_owned(self) -> None:
        current = verification_dag.load_dag(DAG_PATH)
        probe = "core/crates/searchkernel-new/src/lib.rs"
        self.assertFalse(verification_dag.matching_subsystems(current, probe))
        self.assertFalse(verification_dag.matching_exemptions(current, probe))

    def test_new_web_feature_family_is_not_silently_owned(self) -> None:
        current = verification_dag.load_dag(DAG_PATH)
        probe = "searchkernel-web/src/features/new-capability/Page.tsx"
        self.assertFalse(verification_dag.matching_subsystems(current, probe))
        self.assertFalse(verification_dag.matching_exemptions(current, probe))

    def test_ruby_client_family_has_explicit_verification_ownership(self) -> None:
        current = verification_dag.load_dag(DAG_PATH)

        client_owners = verification_dag.matching_subsystems(
            current, "clients/ruby/lib/searchkernel.rb"
        )
        self.assertEqual([owner.id for owner in client_owners], ["client-ruby"])
        self.assertEqual(
            client_owners[0].requires, ("ruby-client", "client-contracts")
        )

        contract_owners = verification_dag.matching_subsystems(
            current, "core/crates/searchkernel-protocol/tests/ruby_json_contract.rs"
        )
        contract_owner_ids = {owner.id for owner in contract_owners}
        self.assertIn("searchkernel-protocol", contract_owner_ids)
        self.assertIn("client-ruby", contract_owner_ids)
        ruby_owner = next(owner for owner in contract_owners if owner.id == "client-ruby")
        self.assertEqual(ruby_owner.requires, ("ruby-client", "client-contracts"))

        task = current.tasks["ruby-client"]
        self.assertEqual(task.kind, "command")
        self.assertEqual(task.command, "./dev test ruby")
        self.assertIn("ruby-client", verification_dag.task_closure(current, ("full",)))

    def test_semantic_vespa_router_targets_are_all_represented_in_dag(self) -> None:
        current = verification_dag.load_dag(DAG_PATH)
        completed = subprocess.run(
            [str(VESPA_ROUTER), "--list"],
            cwd=ROOT,
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            check=True,
        )
        semantic: list[str] = []
        milestones: list[str] = []
        section: str | None = None
        for raw in completed.stdout.splitlines():
            line = raw.strip()
            if line == "Semantic verifier targets:":
                section = "semantic"
                continue
            if line == "Milestone aggregate targets:":
                section = "milestone"
                continue
            if line.endswith(":"):
                section = None
                continue
            if not line:
                continue
            if section == "semantic":
                semantic.append(line)
            elif section == "milestone":
                milestones.append(line)

        self.assertTrue(semantic)
        self.assertEqual(milestones, [f"milestone.m{i}" for i in range(15)])
        for target in [*semantic, *milestones]:
            with self.subTest(target=target):
                self.assertIn(target, current.tasks)

        for target in semantic:
            with self.subTest(command=target):
                self.assertEqual(
                    current.tasks[target].command,
                    f"./dev verify {target}",
                )
        for target in milestones:
            with self.subTest(aggregate=target):
                task = current.tasks[target]
                self.assertEqual(task.kind, "aggregate")
                self.assertTrue(task.provenance)

    def test_semantic_routes_match_expected_wrappers_and_implementations(self) -> None:
        router = VESPA_ROUTER.read_text(encoding="utf-8")
        routes = {
            "vespa.native-sdk": ("verifiers/native-sdk/verify.sh", "reference/vespa/m0/verify-m0.sh"),
            "vespa.rust-ownership": ("verifiers/rust-ownership/verify.sh", "reference/vespa/m1/verify-m1.sh"),
            "vespa.document-mutation": ("verifiers/document-mutation/verify.sh", "reference/vespa/m2/verify-m2.sh"),
            "vespa.linguistics": ("verifiers/linguistics/verify.sh", "reference/vespa/m3/verify-m3.sh"),
            "vespa.text-indexing": ("verifiers/text-indexing/verify.sh", "reference/vespa/m4/verify-m4.sh"),
            "vespa.persistence.reopen": ("verifiers/persistence/reopen/verify.sh", "reference/vespa/m5/verify-m5.sh"),
            "vespa.schema-config": ("verifiers/schema-config/verify.sh", "reference/vespa/m6/verify-m6.sh"),
            "vespa.query-compilation": ("verifiers/query-compilation/verify.sh", "reference/vespa/m7/verify-m7.sh"),
            "vespa.results": ("verifiers/results/verify.sh", "reference/vespa/m8/verify-m8.sh"),
            "vespa.grouping": ("verifiers/grouping/attributes-facets/verify.sh", "reference/vespa/m9/verify-m9.sh"),
            "vespa.grouping.recursive": (
                "verifiers/grouping/recursive/verify.sh",
                "reference/vespa/m9-general-grouping-verifier/verify-recursive-grouping.sh",
            ),
            "vespa.grouping.yql": (
                "verifiers/grouping/yql/verify.sh",
                "reference/vespa/m13/grouping-yql/verify-grouping-yql.sh",
            ),
            "vespa.ranking": ("verifiers/ranking/verify.sh", "reference/vespa/m10/verify-m10.sh"),
            "vespa.ranking.feature-values": (
                "verifiers/ranking/feature-values/verify.sh",
                "reference/vespa/m10/feature-values/verify-feature-values.sh",
            ),
            "vespa.search-reply": (
                "verifiers/search-reply/verify.sh",
                "reference/vespa/searchreply/verify-searchreply.sh",
            ),
            "vespa.hardening": ("verifiers/hardening/full/verify.sh", "reference/vespa/m11/verify-m11.sh"),
            "vespa.hardening.core": (
                "verifiers/hardening/core/verify.sh",
                "reference/vespa/m11/verify-m11-core.sh",
            ),
            "vespa.hardening.stress": (
                "verifiers/hardening/stress/verify.sh",
                "reference/vespa/m11/verify-m11-stress.sh",
            ),
            "vespa.hardening.sanitizers": (
                "verifiers/hardening/sanitizers/verify.sh",
                "reference/vespa/m11/verify-m11-sanitizers.sh",
            ),
            "vespa.nearest-neighbor.query": (
                "verifiers/nearest-neighbor/query/verify.sh",
                "reference/vespa/m12/verify-m12.sh",
            ),
            "vespa.tensor.document-crud": (
                "verifiers/tensor/document-crud/verify.sh",
                "reference/vespa/m13/verify-m13.sh",
            ),
            "vespa.nearest-neighbor.hnsw": (
                "verifiers/nearest-neighbor/hnsw/verify.sh",
                "reference/vespa/m14/verify-m14.sh",
            ),
            "vespa.recovery": ("verifiers/recovery/verify.sh", "reference/vespa/recovery/verify-recovery.sh"),
            "product.rust-real-engine": (
                "verifiers/product/rust-real-engine/verify.sh",
                "reference/vespa/phase10/verify-phase10.sh",
            ),
            "product.integration": (
                "verifiers/product/integration/verify.sh",
                "reference/vespa/i/verify-i.sh",
            ),
        }

        for target, (wrapper_rel, implementation_rel) in routes.items():
            with self.subTest(target=target):
                route = (
                    f"    {target})\n"
                    f'        "$ROOT/{wrapper_rel}"\n'
                    "        ;;"
                )
                self.assertIn(route, router)
                wrapper = (ROOT / "reference" / "vespa" / wrapper_rel).read_text(
                    encoding="utf-8"
                )
                active_wrapper = "\n".join(
                    line
                    for line in wrapper.splitlines()
                    if line.strip() and not line.lstrip().startswith("#")
                )
                self.assertIn(
                    f'exec "$ROOT/{implementation_rel}" "$@"',
                    active_wrapper,
                )

    def test_direct_subsumption_edges_match_frozen_verifier_invocations(self) -> None:
        current = verification_dag.load_dag(DAG_PATH)
        cases = {
            "vespa.rust-ownership": (
                "reference/vespa/m1/verify-m1.sh",
                {"vespa.native-sdk": "m0/verify-m0.sh"},
            ),
            "vespa.document-mutation": (
                "reference/vespa/m2/verify-m2.sh",
                {"vespa.rust-ownership": "m1/verify-m1.sh"},
            ),
            "vespa.linguistics": (
                "reference/vespa/m3/verify-m3.sh",
                {"vespa.document-mutation": "m2/verify-m2.sh"},
            ),
            "vespa.text-indexing": (
                "reference/vespa/m4/verify-m4.sh",
                {"vespa.linguistics": "m3/verify-m3.sh"},
            ),
            "vespa.persistence.reopen": (
                "reference/vespa/m5/verify-m5.sh",
                {"vespa.text-indexing": "m4/verify-m4.sh"},
            ),
            "vespa.schema-config": (
                "reference/vespa/m6/verify-m6.sh",
                {"vespa.persistence.reopen": "m5/verify-m5.sh"},
            ),
            "vespa.query-compilation": (
                "reference/vespa/m7/verify-m7.sh",
                {"vespa.schema-config": "m6/verify-m6.sh"},
            ),
            "vespa.results": (
                "reference/vespa/m8/verify-m8.sh",
                {"vespa.query-compilation": "m7/verify-m7.sh"},
            ),
            "vespa.grouping": (
                "reference/vespa/m9/verify-m9.sh",
                {
                    "vespa.results": "m8/verify-m8.sh",
                    "vespa.grouping.recursive": "m9-general-grouping-verifier/verify-recursive-grouping.sh",
                },
            ),
            "vespa.ranking": (
                "reference/vespa/m10/verify-m10.sh",
                {
                    "vespa.grouping": "m9/verify-m9.sh",
                    "vespa.search-reply": "searchreply/verify-searchreply.sh",
                    "vespa.ranking.feature-values": "feature-values/verify-feature-values.sh",
                },
            ),
            "vespa.hardening": (
                "reference/vespa/m11/verify-m11.sh",
                {
                    "vespa.ranking": "m10/verify-m10.sh",
                    "vespa.hardening.core": "verify-m11-core.sh",
                    "vespa.hardening.stress": "verify-m11-stress.sh",
                    "vespa.hardening.sanitizers": "verify-m11-sanitizers.sh",
                },
            ),
            "vespa.nearest-neighbor.query": (
                "reference/vespa/m12/verify-m12.sh",
                {"vespa.hardening": "m11/verify-m11.sh"},
            ),
            "vespa.tensor.document-crud": (
                "reference/vespa/m13/verify-m13.sh",
                {"vespa.nearest-neighbor.query": "m12/verify-m12.sh"},
            ),
            "vespa.nearest-neighbor.hnsw": (
                "reference/vespa/m14/verify-m14.sh",
                {"vespa.tensor.document-crud": "m13/verify-m13.sh"},
            ),
            "vespa.grouping.yql": (
                "reference/vespa/m13/grouping-yql/verify-grouping-yql.sh",
                {
                    "vespa.grouping.recursive": "m9-general-grouping-verifier/verify-recursive-grouping.sh",
                },
            ),
            "product.integration": (
                "reference/vespa/i/verify-i.sh",
                {
                    "vespa.tensor.document-crud": 'verify-through.sh" m13',
                    "product.rust-real-engine": 'verify-through.sh" phase10',
                },
            ),
            "native-additive-regression": (
                "reference/vespa/phase9/verify-phase9.sh",
                {
                    "vespa.tensor.document-crud": 'verify-through.sh" m13',
                    "vespa.grouping.recursive": "m9-general-grouping-verifier/verify-recursive-grouping.sh",
                },
            ),
        }
        for task_id, (script_path, expected) in cases.items():
            with self.subTest(task=task_id):
                task = current.tasks[task_id]
                self.assertEqual(set(task.subsumes), set(expected))
                script = (ROOT / script_path).read_text(encoding="utf-8")
                active_script = "\n".join(
                    line
                    for line in script.splitlines()
                    if line.strip() and not line.lstrip().startswith("#")
                )
                for proved_task, marker in expected.items():
                    with self.subTest(task=task_id, proves=proved_task):
                        self.assertIn(marker, active_script)

    def test_full_proof_closure_covers_every_semantic_vespa_target(self) -> None:
        current = verification_dag.load_dag(DAG_PATH)
        proof = set(verification_dag.proof_closure(current, current.full_roots))
        semantic = {
            task.id
            for task in current.tasks.values()
            if task.command
            and task.command.startswith("./dev verify ")
            and (task.id.startswith("vespa.") or task.id.startswith("product."))
        }
        self.assertTrue(semantic)
        self.assertFalse(semantic - proof, sorted(semantic - proof))

    def test_m14_is_proven_by_semantic_hnsw_closure_not_an_executable_milestone(self) -> None:
        current = verification_dag.load_dag(DAG_PATH)
        m14 = current.tasks["milestone.m14"]
        self.assertEqual(m14.kind, "aggregate")
        self.assertIsNone(m14.command)
        self.assertEqual(m14.depends_on, ("vespa.nearest-neighbor.hnsw",))
        proof = set(verification_dag.proof_closure(current, ("milestone.m14",)))
        for expected in (
            "vespa.native-sdk",
            "vespa.grouping.recursive",
            "vespa.ranking.feature-values",
            "vespa.search-reply",
            "vespa.hardening.sanitizers",
            "vespa.nearest-neighbor.query",
            "vespa.tensor.document-crud",
            "vespa.nearest-neighbor.hnsw",
        ):
            with self.subTest(expected=expected):
                self.assertIn(expected, proof)


    def test_plan_paths_respects_exemptions_and_override_ownership(self) -> None:
        current = verification_dag.load_dag(DAG_PATH)

        tasks, reasons, unclassified = verification_dag.plan_paths(
            current,
            ("docs/architecture.md",),
        )
        self.assertEqual(tasks, ())
        self.assertEqual(reasons, {})
        self.assertEqual(unclassified, ())

        tasks, reasons, unclassified = verification_dag.plan_paths(
            current,
            ("docs/CAPABILITIES.md",),
        )
        self.assertIn("client-contracts", tasks)
        self.assertIn("portable-ir-boundary", tasks)
        self.assertEqual(unclassified, ())
        self.assertIn(
            "docs/CAPABILITIES.md: subsystem capability-ledger",
            reasons["client-contracts"],
        )
        self.assertIn(
            "docs/CAPABILITIES.md: subsystem capability-ledger",
            reasons["portable-ir-boundary"],
        )

    def test_plan_paths_normalizes_deduplicates_and_fails_closed_on_unknown_paths(self) -> None:
        current = verification_dag.load_dag(DAG_PATH)

        normalized = verification_dag.plan_paths(
            current,
            (
                " clients/php/src/Index.php ",
                r"clients\php\src\Index.php",
                "clients/php/src/Index.php",
            ),
        )
        canonical = verification_dag.plan_paths(
            current,
            ("clients/php/src/Index.php",),
        )
        self.assertEqual(normalized, canonical)

        tasks, reasons, unclassified = verification_dag.plan_paths(
            current,
            ("new-unowned-product/src/lib.rs",),
        )
        self.assertEqual(tasks, ())
        self.assertEqual(reasons, {})
        self.assertEqual(unclassified, ("new-unowned-product/src/lib.rs",))

    def test_legacy_lane_projection_is_derived_from_selected_task_closure(self) -> None:
        current = verification_dag.load_dag(DAG_PATH)
        closure = verification_dag.task_closure(
            current,
            ("php-client", "client-contracts"),
        )

        lanes = verification_dag.legacy_lanes_for_tasks(current, closure)
        selected = {lane for lane, enabled in lanes.items() if enabled}

        self.assertEqual(selected, {"php", "contracts"})
        self.assertFalse(lanes["rust"])
        self.assertFalse(lanes["native"])
        self.assertFalse(lanes["verification"])


    def test_old_schema_version_is_rejected(self) -> None:
        source = DAG_PATH.read_text(encoding="utf-8").replace(
            "schema_version = 2",
            "schema_version = 1",
            1,
        )
        with tempfile.TemporaryDirectory() as temp:
            path = Path(temp) / "dag.toml"
            path.write_text(source, encoding="utf-8")
            with self.assertRaisesRegex(
                verification_dag.DagError,
                "unsupported verification DAG schema_version",
            ):
                verification_dag.load_dag(path)

    def test_unknown_task_reference_is_rejected(self) -> None:
        source = DAG_PATH.read_text(encoding="utf-8")
        mutated = source.replace(
            'requires = ["rust-non-native", "rust-quality"]',
            'requires = ["rust-non-native", "does-not-exist"]',
            1,
        )
        with tempfile.TemporaryDirectory() as temp:
            path = Path(temp) / "dag.toml"
            path.write_text(mutated, encoding="utf-8")
            with self.assertRaisesRegex(verification_dag.DagError, "unknown required task"):
                verification_dag.load_dag(path)

    def test_duplicate_task_id_is_rejected(self) -> None:
        source = DAG_PATH.read_text(encoding="utf-8")
        source += """
[[task]]
id = "rust-non-native"
kind = "aggregate"
runner = "linux"
depends_on = []
cache_result = false
"""
        with tempfile.TemporaryDirectory() as temp:
            path = Path(temp) / "dag.toml"
            path.write_text(source, encoding="utf-8")
            with self.assertRaisesRegex(verification_dag.DagError, "duplicate task id"):
                verification_dag.load_dag(path)

    def test_orphan_aggregate_task_is_rejected(self) -> None:
        source = DAG_PATH.read_text(encoding="utf-8")
        source += """
[[task]]
id = "orphan-aggregate"
kind = "aggregate"
runner = "linux"
depends_on = []
cache_result = false
"""
        with tempfile.TemporaryDirectory() as temp:
            path = Path(temp) / "dag.toml"
            path.write_text(source, encoding="utf-8")
            with self.assertRaisesRegex(
                verification_dag.DagError,
                "tasks are not proved by full_roots",
            ):
                verification_dag.load_dag(path)

    def test_behavioral_subsystem_task_must_be_proved_by_full_root(self) -> None:
        source = DAG_PATH.read_text(encoding="utf-8")
        source += """
[[task]]
id = "orphan-behavior"
kind = "command"
runner = "linux"
command = "true"
depends_on = []
cache_result = false

[[subsystem]]
id = "orphan-subsystem"
paths = ["orphan/**"]
requires = ["orphan-behavior"]
"""
        with tempfile.TemporaryDirectory() as temp:
            path = Path(temp) / "dag.toml"
            path.write_text(source, encoding="utf-8")
            with self.assertRaisesRegex(
                verification_dag.DagError,
                "not proved by full_roots",
            ):
                verification_dag.load_dag(path)

    def test_subsumed_semantic_requirement_need_not_be_scheduled_twice(self) -> None:
        current = verification_dag.load_dag(DAG_PATH)
        execution = set(verification_dag.task_closure(current, current.full_roots))
        proof = set(verification_dag.proof_closure(current, current.full_roots))
        target = "vespa.ranking.feature-values"
        self.assertNotIn(target, execution)
        self.assertIn(target, proof)

        source = DAG_PATH.read_text(encoding="utf-8")
        source += """
[[subsystem]]
id = "proof-only-subsystem"
paths = ["proof-only/**"]
requires = ["vespa.ranking.feature-values"]
"""
        with tempfile.TemporaryDirectory() as temp:
            path = Path(temp) / "dag.toml"
            path.write_text(source, encoding="utf-8")
            verification_dag.load_dag(path)

    def test_proof_subsumption_cycle_is_rejected(self) -> None:
        source = DAG_PATH.read_text(encoding="utf-8")
        source = source.replace(
            'id = "vespa.native-sdk"\nlegacy_lanes = ["native"]',
            'id = "vespa.native-sdk"\nlegacy_lanes = ["native"]',
            1,
        ).replace(
            'id = "vespa.native-sdk"\nlegacy_lanes = ["native"]\nkind = "command"\nrunner = "self-hosted-native"\ncommand = "./dev verify vespa.native-sdk"\ndepends_on = []\nsubsumes = []',
            'id = "vespa.native-sdk"\nlegacy_lanes = ["native"]\nkind = "command"\nrunner = "self-hosted-native"\ncommand = "./dev verify vespa.native-sdk"\ndepends_on = []\nsubsumes = ["vespa.rust-ownership"]',
            1,
        )
        with tempfile.TemporaryDirectory() as temp:
            path = Path(temp) / "dag.toml"
            path.write_text(source, encoding="utf-8")
            with self.assertRaisesRegex(
                verification_dag.DagError,
                "dependency/proof cycle",
            ):
                verification_dag.load_dag(path)

    def test_dependency_cycle_is_rejected(self) -> None:
        source = DAG_PATH.read_text(encoding="utf-8")
        source = source.replace(
            'id = "rust-quality"\nlegacy_lanes = ["rust"]\nkind = "command"\nrunner = "linux"\ncommand = "cargo fmt --all --check && cargo clippy --workspace --all-targets --no-default-features -- -D warnings"\ndepends_on = []',
            'id = "rust-quality"\nlegacy_lanes = ["rust"]\nkind = "command"\nrunner = "linux"\ncommand = "cargo fmt --all --check && cargo clippy --workspace --all-targets --no-default-features -- -D warnings"\ndepends_on = ["rust-non-native"]',
            1,
        )
        with tempfile.TemporaryDirectory() as temp:
            path = Path(temp) / "dag.toml"
            path.write_text(source, encoding="utf-8")
            with self.assertRaisesRegex(verification_dag.DagError, "dependency/proof cycle"):
                verification_dag.load_dag(path)


if __name__ == "__main__":
    unittest.main()
