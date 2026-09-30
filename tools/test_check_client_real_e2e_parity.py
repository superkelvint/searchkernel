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
validate_shared_typed_inputs = PARITY.validate_shared_typed_inputs
validate_observed_intents = PARITY.validate_observed_intents
float_semantic_paths_for_scenario = PARITY.float_semantic_paths_for_scenario

MISUSE_CASES = ("negative_limit_rejected", "negative_offset_rejected")
MISUSE_CONTRACT = {
    "builder_misuse": {
        client: {
            case: (
                {"kind": "not_expressible", "reason": "Rust public API accepts u32."}
                if client == "rust"
                else {"kind": "runtime_rejected"}
            )
            for case in MISUSE_CASES
        }
        for client in PARITY.CLIENTS
    }
}


class RealE2EParityComparatorTests(unittest.TestCase):
    def test_shared_typed_query_scenarios_are_required_and_have_intents(self):
        scenarios = list(PARITY.REQUIRED_SHARED_INTENT_SCENARIOS)
        inputs = {scenario: {"query": {"kind": "contains"}, "result_order": "sorted_ids", "limit": 100} for scenario in scenarios}
        for scenario in ("nearest_neighbor", "hybrid_text_vector"):
            inputs[scenario] = {"query": {"kind": "nearest_neighbor"}, "ranking_profile": "semantic", "query_input_document": "shoe-1", "limit": 3}
        self.assertEqual(inputs, validate_shared_typed_inputs({"scenario_inputs": inputs}, scenarios))
        with self.assertRaisesRegex(SystemExit, "scenario inventory omits required shared-intent scenarios"):
            validate_shared_typed_inputs({"scenario_inputs": inputs}, scenarios[:-1])
        with self.assertRaisesRegex(SystemExit, "omits required shared scenario intents"):
            validate_shared_typed_inputs({"scenario_inputs": {key: value for key, value in inputs.items() if key != "typed_query_geo_box"}}, scenarios)

    def test_client_snapshot_must_echo_the_shared_scenario_intent(self):
        inputs = {scenario: {"query": {"kind": "contains", "value": scenario}, "result_order": "sorted_ids", "limit": 100} for scenario in PARITY.REQUIRED_TYPED_SCENARIOS}
        for scenario in set(PARITY.REQUIRED_SHARED_INTENT_SCENARIOS) - set(inputs):
            inputs[scenario] = {"query": {"kind": "search", "value": scenario}}
        for scenario in ("nearest_neighbor", "hybrid_text_vector"):
            inputs[scenario] = {"query": {"kind": "nearest_neighbor"}, "ranking_profile": "semantic", "query_input_document": "shoe-1", "limit": 3}
        observations = {scenario: {"observation": {"intent": entry, "query_result": ["a"]}}
                       for scenario, entry in inputs.items()}
        validate_observed_intents("rust", observations, inputs)
        observations[PARITY.REQUIRED_TYPED_SCENARIOS[0]]["observation"]["intent"] = {"value": "hard-coded"}
        with self.assertRaisesRegex(SystemExit, "did not consume the shared scenario intent"):
            validate_observed_intents("rust", observations, inputs)

    def test_every_inventory_scenario_needs_a_shared_intent(self):
        corpus = PARITY.json.loads(PARITY.CORPUS.read_text())
        scenarios = list(corpus["scenarios"])
        inputs = dict(corpus["scenario_inputs"])
        del inputs["pagination"]
        with self.assertRaisesRegex(SystemExit, "omits required shared scenario intents: pagination"):
            validate_shared_typed_inputs({"scenario_inputs": inputs}, scenarios)

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

    def test_global_non_comparable_exclusions_are_rejected(self):
        with self.assertRaisesRegex(SystemExit, "global non_comparable"):
            scenario_contract({"scenarios": ["facets"], "non_comparable": {"facets": "reason"}})

    def test_capability_contract_is_per_client_and_keeps_the_scenario_required(self):
        scenarios, capabilities = scenario_contract({
            "scenarios": ["full_text_search", "yql"],
            "client_scenario_contract": {
                "yql": {
                    client: {
                        "negative_limit": (
                            {"kind": "not_expressible", "reason": "Rust type system prevents this input."}
                            if client == "rust"
                            else {"kind": "runtime_rejected"}
                        )
                    }
                    for client in PARITY.CLIENTS
                }
            },
        })
        self.assertEqual(["full_text_search", "yql"], scenarios)
        self.assertEqual("not_expressible", capabilities["yql"]["rust"]["negative_limit"]["kind"])
        with self.assertRaisesRegex(SystemExit, "unknown scenarios"):
            scenario_contract({"scenarios": ["facets"], "client_scenario_contract": {"yql": {}}})
        with self.assertRaisesRegex(SystemExit, "every required client"):
            scenario_contract({"scenarios": ["facets"], "client_scenario_contract": {"facets": {"rust": {"negative_limit": {"kind": "not_expressible", "reason": " "}}}}})

    def test_every_shared_scenario_is_required_in_each_snapshot(self):
        with self.assertRaisesRegex(SystemExit, "python snapshot is missing scenarios: facets"):
            scenario_observations(
                "python",
                {"version": 1, "scenarios": {"full_text_search": {"ids": ["shoe-1"]}}},
                ["full_text_search", "facets"],
                {},
            )
        with self.assertRaisesRegex(SystemExit, "silently skipped"):
            scenario_observations("python", {"version": 1, "scenarios": {"facets": {}}}, ["facets"], {})
        with self.assertRaisesRegex(SystemExit, "silently skipped"):
            scenario_observations("python", {"version": 1, "scenarios": {"facets": []}}, ["facets"], {})
        self.assertEqual(
            {"facets": ["Acme", "TrailCo"], "runtime_flag": False},
            scenario_observations(
                "python",
                {"version": 1, "scenarios": {"facets": ["Acme", "TrailCo"], "runtime_flag": False}},
                ["facets", "runtime_flag"],
                {},
            ),
        )

    def test_not_expressible_requires_exact_contract_observation(self):
        contract = MISUSE_CONTRACT
        observation = {"version": 1, "scenarios": {
            "builder_misuse": {
                "observation": {"page_and_limit_rejected": True},
                "capabilities": contract["builder_misuse"]["rust"],
            }
        }}
        self.assertEqual(
            observation["scenarios"],
            scenario_observations("rust", observation, ["builder_misuse"], contract),
        )
        python_observation = {"version": 1, "scenarios": {
            "builder_misuse": {
                "observation": {
                    "page_and_limit_rejected": True,
                    "negative_limit_rejected": True,
                    "negative_offset_rejected": True,
                },
                "capabilities": contract["builder_misuse"]["python"],
            }
        }}
        self.assertEqual(
            python_observation["scenarios"],
            scenario_observations("python", python_observation, ["builder_misuse"], contract),
        )
        python_observation["scenarios"]["builder_misuse"]["observation"]["negative_limit_rejected"] = False
        with self.assertRaisesRegex(SystemExit, "runtime rejection was not observed"):
            scenario_observations("python", python_observation, ["builder_misuse"], contract)
        with self.assertRaisesRegex(SystemExit, "unchecked"):
            scenario_observations("python", {"version": 1, "scenarios": {"builder_misuse": {
                "observation": {"negative_limit_rejected": True},
                "capabilities": {"negative_limit_rejected": {"kind": "not_expressible", "reason": "unknown"}},
            }}}, ["builder_misuse"], contract)

    def test_equal_structures_pass(self):
        self.assertEqual([], compare({"ids": ["a", "b"], "count": 2}, {"ids": ["a", "b"], "count": 2}))

    def test_float_tolerance_is_relative_and_absolute(self):
        self.assertEqual([], compare({"score": 100.0}, {"score": 100.00005}))
        self.assertTrue(compare({"score": 100.0}, {"score": 100.01}))
        self.assertEqual([], compare(100.0, 100.00005))
        self.assertTrue(compare(100.0, 100.01))
        self.assertEqual([], compare({"scores": [100.0]}, {"scores": [100.00005]}))

    def test_integer_semantics_reject_float_drift_and_float_kind(self):
        self.assertTrue(compare(7, 7.0))
        self.assertTrue(compare(7, 7.0000005))
        self.assertTrue(compare({"integer_value": 7}, {"integer_value": 7.0}))
        self.assertTrue(compare({"integer_value": 7}, {"integer_value": 7.0000005}))
        self.assertTrue(compare({"sum": 7}, {"sum": 7.0}))
        self.assertTrue(compare({"sum": 7}, {"sum": 7.0000005}))
        self.assertEqual([], compare({"count": 7}, {"count": 7}))

    def test_close_float_values_pass_and_material_drift_fails(self):
        self.assertEqual([], compare({"score": 7.0}, {"score": 7.0000005}))
        self.assertTrue(compare({"score": 7.0}, {"score": 7.01}))
        for invalid in (float("nan"), float("inf"), float("-inf")):
            self.assertTrue(compare({"score": 1.0}, {"score": invalid}))

    def test_float_semantics_for_typed_query_intents_come_from_the_schema(self):
        inputs = {"typed_query_range": {"query": {"kind": "range", "field": "price"}}}
        schema = {"fields": {"price": {"type": "float64"}}}
        paths = float_semantic_paths_for_scenario("typed_query_range", inputs, schema)
        self.assertEqual(
            [],
            compare(
                {"intent": {"query": {"lower": 50.0}}},
                {"intent": {"query": {"lower": 50}}},
                float_semantic_paths=paths,
            ),
        )
        schema["fields"]["price"]["type"] = "int64"
        self.assertFalse(float_semantic_paths_for_scenario("typed_query_range", inputs, schema))

    def test_integer_and_boolean_kinds_remain_exact(self):
        self.assertTrue(compare(True, 1))
        self.assertTrue(compare(False, 0))
        self.assertTrue(compare({"count": 1}, {"count": 2}))
        self.assertTrue(compare({"count": 1}, {"count": True}))
        self.assertTrue(compare({"count": 0}, {"count": False}))

    def test_missing_fields_and_ordered_ids_fail(self):
        self.assertTrue(compare({"ids": ["a", "b"]}, {"ids": ["a"]}))
        self.assertTrue(compare({"ids": ["a", "b"]}, {"ids": ["b", "a"]}))


if __name__ == "__main__":
    unittest.main()
