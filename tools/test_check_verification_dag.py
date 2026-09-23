#!/usr/bin/env python3
"""Regression tests for tools/check-verification-dag.py."""

from __future__ import annotations

import importlib.util
import tempfile
import textwrap
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
CHECKER = ROOT / "tools/check-verification-dag.py"
SPEC = importlib.util.spec_from_file_location("check_verification_dag", CHECKER)
assert SPEC and SPEC.loader
CHECK = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(CHECK)


def parse(source: str):
    return CHECK.tomllib.loads(textwrap.dedent(source))


MINIMAL = """
schema_version = 1
unknown_behavior = "block"
runner_classes = ["linux", "mixed"]
full_roots = ["full"]

[[task]]
id = "leaf"
kind = "command"
runner = "linux"
command = "python3"
depends_on = []
cache_result = false

[[task]]
id = "full"
kind = "aggregate"
runner = "mixed"
depends_on = ["leaf"]
cache_result = false

[[subsystem]]
id = "example"
paths = ["example/**"]
requires = ["leaf"]
"""


class VerificationDagTests(unittest.TestCase):
    def validate(self, source: str):
        with tempfile.TemporaryDirectory() as temp:
            return CHECK.validate(
                parse(source),
                repo_root=Path(temp),
                structure_only=True,
            )

    def assert_invalid(self, source: str, needle: str) -> None:
        with self.assertRaises(CHECK.DagError) as ctx:
            self.validate(source)
        self.assertIn(needle, str(ctx.exception))

    def test_minimal_valid_graph(self) -> None:
        tasks, subsystems = self.validate(MINIMAL)
        self.assertEqual(set(tasks), {"leaf", "full"})
        self.assertEqual(set(subsystems), {"example"})

    def test_duplicate_task_id_fails_closed(self) -> None:
        self.assert_invalid(
            MINIMAL.replace(
                '[[task]]\nid = "full"',
                '[[task]]\nid = "leaf"\nkind = "command"\nrunner = "linux"\n'
                'command = "python3"\ndepends_on = []\ncache_result = false\n\n'
                '[[task]]\nid = "full"',
            ),
            "duplicate task id: leaf",
        )

    def test_unknown_dependency_fails_closed(self) -> None:
        self.assert_invalid(
            MINIMAL.replace('depends_on = ["leaf"]', 'depends_on = ["missing"]', 1),
            "references unknown task: missing",
        )

    def test_cycle_fails_closed(self) -> None:
        source = MINIMAL.replace(
            'depends_on = []\ncache_result = false\n\n[[task]]\nid = "full"',
            'depends_on = ["full"]\ncache_result = false\n\n[[task]]\nid = "full"',
        )
        self.assert_invalid(source, "verification DAG cycle")

    def test_command_task_requires_command(self) -> None:
        self.assert_invalid(
            MINIMAL.replace('command = "python3"\n', ""),
            "command task leaf must define command",
        )

    def test_aggregate_cannot_hide_executable_command(self) -> None:
        source = MINIMAL.replace(
            'id = "full"\nkind = "aggregate"\nrunner = "mixed"',
            'id = "full"\nkind = "aggregate"\nrunner = "mixed"\ncommand = "true"',
        )
        self.assert_invalid(source, "aggregate task full must not define command")

    def test_unverified_task_requires_reason(self) -> None:
        source = MINIMAL.replace(
            'id = "leaf"\nkind = "command"\nrunner = "linux"\ncommand = "python3"',
            'id = "leaf"\nkind = "unverified"\nrunner = "linux"',
        )
        self.assert_invalid(source, "unverified task leaf must explain reason")

    def test_required_task_must_be_reachable_from_full_root(self) -> None:
        source = MINIMAL + """
[[task]]
id = "orphan"
kind = "command"
runner = "linux"
command = "python3"
depends_on = []
cache_result = false
"""
        self.assert_invalid(source, "required tasks are unreachable from full_roots: orphan")

    def test_optional_compatibility_alias_task_may_be_outside_full_root(self) -> None:
        source = MINIMAL + """
[[task]]
id = "compat.old"
kind = "aggregate"
runner = "linux"
depends_on = ["leaf"]
required_in_full = false
cache_result = false
"""
        tasks, _ = self.validate(source)
        self.assertIn("compat.old", tasks)

    def test_subsumes_contributes_to_closure(self) -> None:
        source = MINIMAL.replace(
            'depends_on = ["leaf"]',
            'depends_on = ["wrapper"]',
            1,
        ) + """
[[task]]
id = "wrapper"
kind = "command"
runner = "linux"
command = "python3"
depends_on = []
subsumes = ["leaf"]
cache_result = false
"""
        tasks, _ = self.validate(source)
        self.assertIn("wrapper", tasks)

    def test_behavioral_subsystem_cannot_have_empty_verification(self) -> None:
        self.assert_invalid(
            MINIMAL.replace('requires = ["leaf"]', "requires = []"),
            "behavioral subsystem example must require verification tasks",
        )

    def test_nonbehavioral_subsystem_may_have_empty_verification(self) -> None:
        source = MINIMAL.replace(
            'requires = ["leaf"]',
            "requires = []\nbehavioral = false",
        )
        _, subsystems = self.validate(source)
        self.assertFalse(subsystems["example"]["behavioral"])

    def test_unknown_fields_fail_closed(self) -> None:
        self.assert_invalid(
            MINIMAL.replace('cache_result = false', 'cache_result = false\ntyop = true', 1),
            "unknown fields: tyop",
        )

    def test_repository_dag_is_valid(self) -> None:
        data = CHECK.tomllib.loads(
            (ROOT / "verification/dag.toml").read_text(encoding="utf-8")
        )
        tasks, subsystems = CHECK.validate(data, repo_root=ROOT)
        self.assertGreater(len(tasks), 30)
        self.assertGreater(len(subsystems), 20)


if __name__ == "__main__":
    unittest.main()
