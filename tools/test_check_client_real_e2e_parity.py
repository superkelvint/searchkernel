import importlib.util
from pathlib import Path
import unittest

SPEC = importlib.util.spec_from_file_location(
    "real_e2e_parity", Path(__file__).with_name("check-client-real-e2e-parity.py")
)
assert SPEC is not None and SPEC.loader is not None
PARITY = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(PARITY)
compare = PARITY.compare
scenario_ids = PARITY.scenario_ids
scenario_contract = PARITY.scenario_contract
scenario_observations = PARITY.scenario_observations


class RealE2EParityComparatorTests(unittest.TestCase):
    def test_checker_uses_the_shared_scenario_list(self):
        self.assertEqual(
            ["full_text_search", "facets"],
            scenario_ids({"scenarios": ["full_text_search", "facets"]}),
        )

    def test_checker_rejects_the_old_separate_parity_list(self):
        with self.assertRaisesRegex(SystemExit, "scenarios must"):
            scenario_ids({"parity_scenarios": ["facets"]})

    def test_scenario_ids_must_be_unique_and_nonempty(self):
        with self.assertRaisesRegex(SystemExit, "unique"):
            scenario_ids({"scenarios": ["facets", "facets"]})
        with self.assertRaisesRegex(SystemExit, "non-empty scenario IDs"):
            scenario_ids({"scenarios": ["facets", ""]})

    def test_only_explicitly_classified_scenarios_are_excluded(self):
        comparable, excluded = scenario_contract({
            "scenarios": ["full_text_search", "yql"],
            "non_comparable": {"yql": "Rust facade does not expose YQL."},
        })
        self.assertEqual(["full_text_search"], comparable)
        self.assertEqual({"yql": "Rust facade does not expose YQL."}, excluded)
        with self.assertRaisesRegex(SystemExit, "unknown scenarios"):
            scenario_contract({"scenarios": ["facets"], "non_comparable": {"yql": "reason"}})
        with self.assertRaisesRegex(SystemExit, "non-empty rationales"):
            scenario_contract({"scenarios": ["facets"], "non_comparable": {"facets": " "}})

    def test_every_shared_scenario_is_required_in_each_snapshot(self):
        with self.assertRaisesRegex(SystemExit, "python snapshot is missing scenarios: facets"):
            scenario_observations(
                "python",
                {"version": 1, "scenarios": {"full_text_search": {"ids": ["shoe-1"]}}},
                ["full_text_search", "facets"],
            )

    def test_equal_structures_pass(self):
        self.assertEqual([], compare({"ids": ["a", "b"], "count": 2}, {"ids": ["a", "b"], "count": 2}))

    def test_float_tolerance_is_relative_and_absolute(self):
        self.assertEqual([], compare({"score": 100.0}, {"score": 100.00005}))
        self.assertTrue(compare({"score": 100.0}, {"score": 100.01}))

    def test_integer_and_boolean_kinds_remain_exact(self):
        self.assertTrue(compare({"count": 1}, {"count": 2}))
        self.assertTrue(compare({"count": 1}, {"count": True}))

    def test_missing_fields_and_ordered_ids_fail(self):
        self.assertTrue(compare({"ids": ["a", "b"]}, {"ids": ["a"]}))
        self.assertTrue(compare({"ids": ["a", "b"]}, {"ids": ["b", "a"]}))


if __name__ == "__main__":
    unittest.main()
