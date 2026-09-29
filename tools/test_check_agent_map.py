#!/usr/bin/env python3
from __future__ import annotations

import copy
import importlib.util
import pathlib
import unittest

ROOT = pathlib.Path(__file__).resolve().parents[1]
MODULE_PATH = ROOT / "tools" / "check-agent-map.py"
SPEC = importlib.util.spec_from_file_location("check_agent_map", MODULE_PATH)
assert SPEC and SPEC.loader
MODULE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MODULE)


class AgentMapCoverageTests(unittest.TestCase):
    def setUp(self):
        self.data = MODULE.load_registry()
        self.tracked = MODULE.tracked_paths()

    def test_committed_map_covers_current_structural_boundaries(self):
        boundaries = MODULE.structural_boundaries(
            self.tracked, MODULE.workspace_members()
        )
        self.assertEqual(MODULE.missing_boundaries(self.data, boundaries), [])

    def test_new_core_crate_is_autofixed_from_workspace_template(self):
        data = copy.deepcopy(self.data)
        path = "core/crates/searchkernel-new/"
        boundaries = [MODULE.Boundary("workspace", path)]
        tracked = set(self.tracked)
        tracked.add(path + "README.md")

        result, added, unresolved = MODULE.synchronized_registry(
            data, boundaries, tracked
        )

        self.assertEqual(added, [path])
        self.assertEqual(unresolved, [])
        meta = result["change_areas"][path]
        self.assertEqual(meta["owner"], "searchkernel-new")
        self.assertEqual(meta["read"], [path + "README.md"])
        self.assertEqual(meta["focused_check"], "command:check-fast")

    def test_new_client_is_autofixed_when_test_target_exists(self):
        data = copy.deepcopy(self.data)
        data["test"]["go"] = {"description": "Go client tests"}
        path = "clients/go/"
        result, added, unresolved = MODULE.synchronized_registry(
            data,
            [MODULE.Boundary("client", path)],
            self.tracked | {path + "README.md"},
        )

        self.assertEqual(added, [path])
        self.assertEqual(unresolved, [])
        meta = result["change_areas"][path]
        self.assertEqual(meta["test"], ["test:go"])
        self.assertEqual(meta["strong_check"], "test:go")

    def test_new_client_without_test_target_requires_a_decision(self):
        result, added, unresolved = MODULE.synchronized_registry(
            copy.deepcopy(self.data),
            [MODULE.Boundary("client", "clients/go/")],
            self.tracked,
        )
        self.assertEqual(added, [])
        self.assertTrue(unresolved)
        self.assertNotIn("clients/go/", result["change_areas"])

    def test_new_top_level_area_is_never_invented(self):
        result, added, unresolved = MODULE.synchronized_registry(
            copy.deepcopy(self.data),
            [MODULE.Boundary("top-level", "replication/")],
            self.tracked,
        )
        self.assertEqual(added, [])
        self.assertTrue(unresolved)
        self.assertNotIn("replication/", result["change_areas"])


if __name__ == "__main__":
    unittest.main()
