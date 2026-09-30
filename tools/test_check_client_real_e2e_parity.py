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
        corpus = PARITY.json.loads(PARITY.CORPUS.read_text())
        scenarios = list(corpus["scenarios"])
        inputs = dict(corpus["scenario_inputs"])
        self.assertEqual(inputs, validate_shared_typed_inputs({"scenario_inputs": inputs}, scenarios))
        with self.assertRaisesRegex(SystemExit, "scenario inventory omits required shared-intent scenarios"):
            validate_shared_typed_inputs({"scenario_inputs": inputs}, scenarios[:-1])
        with self.assertRaisesRegex(SystemExit, "omits required shared scenario intents"):
            validate_shared_typed_inputs(
                {"scenario_inputs": {key: value for key, value in inputs.items() if key != "typed_query_geo_box"}},
                scenarios,
            )
        ast_inputs = dict(inputs)
        ast_inputs["typed_query_contains"] = {
            "query": {"kind": "contains", "field": "title", "value": "quick"},
            "result_order": "sorted_ids",
            "limit": 100,
        }
        with self.assertRaisesRegex(SystemExit, "flat scenario-specific"):
            validate_shared_typed_inputs({"scenario_inputs": ast_inputs}, scenarios)

    def test_v1_required_inventory_is_independent_from_the_corpus(self):
        corpus = PARITY.json.loads(PARITY.CORPUS.read_text())
        self.assertTrue(set(PARITY.REQUIRED_V1_SCENARIOS).issubset(set(corpus["scenarios"])))
        shrunken = [scenario for scenario in corpus["scenarios"] if scenario != "advanced_update"]
        with self.assertRaisesRegex(SystemExit, "scenario inventory omits required shared-intent scenarios: advanced_update"):
            validate_shared_typed_inputs({"scenario_inputs": corpus["scenario_inputs"]}, shrunken)

    def test_client_snapshot_must_echo_the_shared_scenario_intent(self):
        corpus = PARITY.json.loads(PARITY.CORPUS.read_text())
        inputs = corpus["scenario_inputs"]
        observations = {
            scenario: {"observation": {"intent": entry, "query_result": ["a"]}}
            for scenario, entry in inputs.items()
        }
        validate_observed_intents("rust", observations, inputs)
        observations[PARITY.REQUIRED_TYPED_SCENARIOS[0]]["observation"]["intent"] = {"value": "hard-coded"}
        with self.assertRaisesRegex(SystemExit, "did not consume the shared scenario intent"):
            validate_observed_intents("rust", observations, inputs)

    def test_shared_intent_sentinels_expose_hard_coded_request_defaults(self):
        corpus = PARITY.json.loads(PARITY.CORPUS.read_text())
        inputs = corpus["scenario_inputs"]
        self.assertEqual("asc", inputs["sorting_projection"]["sort"][0]["direction"])
        self.assertEqual("desc", inputs["pagination"]["sort"][0]["direction"])
        self.assertEqual("kept_products", inputs["group_keep_filter"]["outputs"][0]["name"])
        self.assertEqual({"needle": "shoes"}, inputs["yql"]["bindings"])
        self.assertFalse(inputs["typed_query_range_exclusive"]["include_upper"])

    def test_parity_writers_use_execution_consumed_intents_not_corpus_decoration(self):
        root = Path(__file__).parents[1]
        checks = {
            "clients/python/examples/try_searchkernel_client_api_e2e.py": (
                "PARITY_CONSUMED_INTENTS",
                'E2E_CORPUS["scenario_inputs"].get(scenario)',
            ),
            "clients/typescript/examples/try_searchkernel_client_api_e2e.ts": (
                "PARITY_CONSUMED_INTENTS",
                "E2E_CORPUS.scenario_inputs[scenario]",
            ),
            "clients/ruby/examples/try_searchkernel_client_api_e2e.rb": (
                "PARITY_CONSUMED_INTENTS",
                'E2E_CORPUS.fetch("scenario_inputs")[scenario]',
            ),
            "clients/php/examples/try_searchkernel_client_api_e2e.php": (
                "$parityConsumedIntents",
                "searchkernelPhpClientApiE2eCorpus()['scenario_inputs']",
            ),
            "core/crates/searchkernel/examples/try_searchkernel_client_api_e2e.rs": (
                ".get(scenario)",
                "for (scenario, intent) in shared_inputs",
            ),
        }
        for relative, (consumed_marker, forbidden) in checks.items():
            source = (root / relative).read_text()
            self.assertIn(consumed_marker, source, relative)
            self.assertNotIn(forbidden, source, relative)

        generic_interpreters = {
            "clients/python/examples/try_searchkernel_client_api_e2e.py": "def shared_typed_query(spec)",
            "clients/typescript/examples/try_searchkernel_client_api_e2e.ts": "function sharedTypedQuery(spec",
            "clients/ruby/examples/try_searchkernel_client_api_e2e.rb": "def shared_typed_query(spec)",
            "clients/php/examples/try_searchkernel_client_api_e2e.php": "function sharedTypedQuery(array $spec)",
            "core/crates/searchkernel/examples/try_searchkernel_client_api_e2e.rs": "fn query_from_spec(",
        }
        for relative, forbidden in generic_interpreters.items():
            self.assertNotIn(forbidden, (root / relative).read_text(), relative)

        corpus = PARITY.json.loads(PARITY.CORPUS.read_text())
        for scenario in PARITY.REQUIRED_TYPED_SCENARIOS:
            intent = corpus["scenario_inputs"][scenario]
            self.assertNotIn("query", intent, scenario)
            self.assertNotIn("kind", intent, scenario)
            self.assertNotIn("clauses", intent, scenario)

    def test_incomplete_executed_intent_fails_even_when_results_match(self):
        inputs = {
            "facets": {
                "query": "shoes",
                "field": "brand",
                "name": "brands",
                "limit": 10,
                "sort": "count_desc",
                "page_limit": 20,
            }
        }
        observations = {
            "facets": {
                "intent": {
                    "query": "shoes",
                    "field": "brand",
                    "name": "brands",
                    "limit": 10,
                    "page_limit": 20,
                },
                "Acme": 2,
            }
        }
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

    def test_unsafe_javascript_integer_decimal_strings_compare_exactly(self):
        unsafe = 9007199254740993
        self.assertEqual([], compare(unsafe, "9007199254740993"))
        self.assertEqual([], compare("9007199254740993", unsafe))
        self.assertTrue(compare(unsafe, "9007199254740992"))
        self.assertTrue(compare(42, "42"))
        self.assertTrue(compare(True, "9007199254740993"))
        self.assertTrue(compare(9007199254740993.0, "9007199254740993"))

    def test_close_float_values_pass_and_material_drift_fails(self):
        self.assertEqual([], compare({"score": 7.0}, {"score": 7.0000005}))
        self.assertTrue(compare({"score": 7.0}, {"score": 7.01}))
        for invalid in (float("nan"), float("inf"), float("-inf")):
            self.assertTrue(compare({"score": 1.0}, {"score": invalid}))

    def test_float_semantics_for_typed_query_intents_come_from_the_schema(self):
        inputs = {"typed_query_range": {"field": "price", "lower": 50.0, "upper": 110.0}}
        schema = {"fields": {"price": {"type": "float64"}}}
        paths = float_semantic_paths_for_scenario("typed_query_range", inputs, schema)
        self.assertEqual(
            [],
            compare(
                {"intent": {"lower": 50.0}},
                {"intent": {"lower": 50}},
                float_semantic_paths=paths,
            ),
        )
        schema["fields"]["price"]["type"] = "int64"
        paths = float_semantic_paths_for_scenario("typed_query_range", inputs, schema)
        self.assertFalse(paths)
        self.assertTrue(
            compare(
                {"intent": {"lower": 7}},
                {"intent": {"lower": 7.0000005}},
                float_semantic_paths=paths,
            )
        )

    def test_float_semantic_paths_accept_string_search_queries(self):
        self.assertEqual(
            frozenset(),
            float_semantic_paths_for_scenario(
                "full_text_search",
                {"full_text_search": {"query": "shoes"}},
                {"fields": {"price": {"type": "float64"}}},
            ),
        )

    def test_filter_binding_float_tolerance_comes_from_the_referenced_field(self):
        inputs = {"text_filter_binding": {"filter": "price <= :max", "bindings": {"max": 7.0}}}
        schema = {"fields": {"price": {"type": "float64"}, "inventory": {"type": "int64"}}}
        paths = float_semantic_paths_for_scenario("text_filter_binding", inputs, schema)
        self.assertIn("$.intent.bindings.max", paths)
        self.assertEqual(
            [],
            compare(
                {"intent": {"bindings": {"max": 7}}},
                {"intent": {"bindings": {"max": 7.0000005}}},
                float_semantic_paths=paths,
            ),
        )
        inputs["text_filter_binding"] = {"filter": "inventory <= :max", "bindings": {"max": 7}}
        paths = float_semantic_paths_for_scenario("text_filter_binding", inputs, schema)
        self.assertNotIn("$.intent.bindings.max", paths)
        self.assertTrue(
            compare(
                {"intent": {"bindings": {"max": 7}}},
                {"intent": {"bindings": {"max": 7.0000005}}},
                float_semantic_paths=paths,
            )
        )

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
