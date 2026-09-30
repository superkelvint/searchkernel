#!/usr/bin/env python3
"""Run the full real-engine showcases and compare shared semantic observations."""
from __future__ import annotations

import json
import math
import os
from pathlib import Path
import re
import subprocess
import sys
import tempfile
from typing import Any

ROOT = Path(__file__).resolve().parents[1]
CORPUS = ROOT / "reference/client-real-e2e-parity/corpus.json"
CLIENTS = ("rust", "python", "typescript", "php", "ruby")
FLOAT_TOLERANCE = 1e-6
FLOAT_SEMANTIC_KEYS = {
    # Explicitly floating output paths. JavaScript can serialize an integral
    # float (3.0) as the JSON number 3, so these paths use their domain type.
    "price", "sale_price", "avg_price", "sum_price", "min_price", "max_price",
    "revenue", "score", "scores", "top_score", "feature", "relevance",
}
REQUIRED_TYPED_SCENARIOS = (
    "typed_query_contains", "typed_query_phrase", "typed_query_near", "typed_query_ordered_near", "typed_query_range",
    "typed_query_boolean", "typed_query_in", "typed_query_equals",
    "typed_query_fuzzy", "typed_query_weak_and", "typed_query_wand",
    "typed_query_weighted_set", "typed_query_dot_product",
    "typed_query_same_element", "typed_query_geo_location", "typed_query_geo_box",
)
REQUIRED_SHARED_INTENT_SCENARIOS = (*REQUIRED_TYPED_SCENARIOS, "nearest_neighbor", "hybrid_text_vector")


def compare(
    expected: Any,
    actual: Any,
    path: str = "$",
    differences: list[str] | None = None,
    float_semantic_paths: frozenset[str] = frozenset(),
) -> list[str]:
    """Collect structural differences, allowing only small finite float drift."""
    if differences is None:
        differences = []
    if isinstance(expected, dict) and isinstance(actual, dict):
        if expected.keys() != actual.keys():
            differences.append(f"{path}: keys differ: {sorted(expected)} != {sorted(actual)}")
            return differences
        for key in sorted(expected):
            compare(expected[key], actual[key], f"{path}.{key}", differences, float_semantic_paths)
        return differences
    if isinstance(expected, list) and isinstance(actual, list):
        if len(expected) != len(actual):
            differences.append(f"{path}: lengths differ: {len(expected)} != {len(actual)}")
            return differences
        for index, (left, right) in enumerate(zip(expected, actual)):
            compare(left, right, f"{path}[{index}]", differences, float_semantic_paths)
        return differences
    if type(expected) is bool or type(actual) is bool:
        if type(expected) is not type(actual) or expected != actual:
            differences.append(f"{path}: {expected!r} != {actual!r}")
        return differences
    if isinstance(expected, (int, float)) and isinstance(actual, (int, float)):
        path_keys = set(re.split(r"[.\[\]]+", path))
        float_semantics = path in float_semantic_paths or bool(path_keys & FLOAT_SEMANTIC_KEYS)
        if type(expected) is not type(actual) and not float_semantics:
            differences.append(f"{path}: numeric types differ, got {type(expected).__name__} != {type(actual).__name__}")
            return differences
        if isinstance(expected, int) and isinstance(actual, int):
            if expected != actual:
                differences.append(f"{path}: {expected!r} != {actual!r}")
        elif not (math.isfinite(expected) and math.isfinite(actual)) or not math.isclose(
            expected, actual, rel_tol=FLOAT_TOLERANCE, abs_tol=FLOAT_TOLERANCE
        ):
            differences.append(f"{path}: {expected!r} != {actual!r} (tolerance {FLOAT_TOLERANCE:g})")
        return differences
    if type(expected) is not type(actual) or expected != actual:
        differences.append(f"{path}: {expected!r} != {actual!r}")
    return differences


def run(command: list[str], env: dict[str, str]) -> None:
    print("+", " ".join(command), flush=True)
    subprocess.run(command, cwd=ROOT, env=env, check=True)


def scenario_ids(corpus: dict[str, Any]) -> list[str]:
    """Return the single scenario contract shared by the corpus and checker."""
    scenarios = corpus.get("scenarios")
    if not isinstance(scenarios, list) or not scenarios or any(
        not isinstance(item, str) or not item for item in scenarios
    ):
        raise SystemExit("scenarios must contain non-empty scenario IDs")
    if len(scenarios) != len(set(scenarios)):
        raise SystemExit("scenario IDs must be unique")
    return scenarios


def scenario_contract(corpus: dict[str, Any]) -> tuple[list[str], dict[str, dict[str, dict[str, dict[str, str]]]]]:
    scenarios = scenario_ids(corpus)
    if "non_comparable" in corpus:
        raise SystemExit("global non_comparable scenario exclusions are forbidden")
    capabilities = corpus.get("client_scenario_contract", {})
    if not isinstance(capabilities, dict):
        raise SystemExit("client_scenario_contract must be an object")
    unknown = sorted(set(capabilities) - set(scenarios))
    if unknown:
        raise SystemExit(f"client_scenario_contract references unknown scenarios: {', '.join(unknown)}")
    for scenario, clients in capabilities.items():
        if not isinstance(clients, dict):
            raise SystemExit(f"{scenario}: client contract must be an object")
        if set(clients) != set(CLIENTS):
            raise SystemExit(f"{scenario}: every required client needs an explicit per-case classification")
        cases = set()
        for client, entry in clients.items():
            if not isinstance(entry, dict) or not entry:
                raise SystemExit(f"{scenario}.{client}: client case contract must be a non-empty object")
            cases.update(entry)
        for client, entry in clients.items():
            if set(entry) != cases:
                raise SystemExit(f"{scenario}.{client}: missing per-case capability classification")
            for case, classification in entry.items():
                if not isinstance(case, str) or not case or not isinstance(classification, dict):
                    raise SystemExit(f"{scenario}.{client}.{case}: invalid case classification")
                kind = classification.get("kind")
                if kind == "not_expressible":
                    if not isinstance(classification.get("reason"), str) or not classification["reason"].strip():
                        raise SystemExit(f"{scenario}.{client}.{case}: not_expressible requires a concrete reason")
                elif kind not in {"runtime_rejected", "comparable"}:
                    raise SystemExit(f"{scenario}.{client}.{case}: unsupported case classification")
    return scenarios, capabilities


def validate_shared_typed_inputs(corpus: dict[str, Any], scenarios: list[str]) -> dict[str, Any]:
    missing = sorted(set(REQUIRED_SHARED_INTENT_SCENARIOS) - set(scenarios))
    if missing:
        raise SystemExit(f"shared corpus omits required shared-intent scenarios: {', '.join(missing)}")
    inputs = corpus.get("scenario_inputs")
    if not isinstance(inputs, dict):
        raise SystemExit("shared corpus scenario_inputs must be an object")
    missing_inputs = sorted(set(REQUIRED_SHARED_INTENT_SCENARIOS) - inputs.keys())
    if missing_inputs:
        raise SystemExit(f"shared corpus omits required shared scenario intents: {', '.join(missing_inputs)}")
    for scenario in REQUIRED_TYPED_SCENARIOS:
        entry = inputs[scenario]
        if not isinstance(entry, dict) or not isinstance(entry.get("query"), dict) or entry.get("result_order") not in {"sorted_ids", "ranked"}:
            raise SystemExit(f"{scenario}: shared query intent or result ordering is invalid")
    for scenario in ("nearest_neighbor", "hybrid_text_vector"):
        entry = inputs[scenario]
        if (not isinstance(entry, dict) or not isinstance(entry.get("query"), dict)
                or not isinstance(entry.get("ranking_profile"), str)
                or not isinstance(entry.get("query_input_document"), str)
                or type(entry.get("limit")) is not int or entry["limit"] < 1):
            raise SystemExit(f"{scenario}: query, ranking, vector document, and limit intent are required")
    return inputs


def validate_observed_intents(client: str, observations: dict[str, Any], inputs: dict[str, Any]) -> None:
    for scenario in REQUIRED_SHARED_INTENT_SCENARIOS:
        value = observations.get(scenario)
        payload = value.get("observation", value) if isinstance(value, dict) else None
        expected = inputs[scenario]["query"] if scenario in REQUIRED_TYPED_SCENARIOS else inputs[scenario]
        if not isinstance(payload, dict) or payload.get("intent") != expected:
            raise SystemExit(f"{client}.{scenario} did not consume the shared scenario intent")


def scenario_observations(
    client: str,
    trace: dict[str, Any],
    scenarios: list[str],
    capabilities: dict[str, dict[str, dict[str, dict[str, str]]]],
) -> dict[str, Any]:
    observed = trace.get("scenarios")
    if trace.get("version") != 1 or not isinstance(observed, dict):
        raise SystemExit(f"{client} snapshot has an invalid version or scenarios object")
    missing = sorted(set(scenarios) - observed.keys())
    if missing:
        raise SystemExit(f"{client} snapshot is missing scenarios: {', '.join(missing)}")
    for scenario in scenarios:
        value = observed[scenario]
        if value is None or (isinstance(value, (dict, list, str)) and not value):
            raise SystemExit(f"{client}.{scenario} has an empty observation and appears silently skipped")
        entry = capabilities.get(scenario, {}).get(client)
        if isinstance(value, dict) and set(value) == {"observation", "capabilities"}:
            if not isinstance(value["observation"], dict) or value["capabilities"] != (entry or {}):
                raise SystemExit(f"{client}.{scenario} has a missing or unchecked per-case capability classification")
            for case, classification in (entry or {}).items():
                observed_case = case in value["observation"]
                if classification["kind"] == "runtime_rejected":
                    if not observed_case or value["observation"][case] is not True:
                        raise SystemExit(f"{client}.{scenario}.{case} runtime rejection was not observed")
                elif observed_case:
                    if classification["kind"] == "not_expressible":
                        raise SystemExit(f"{client}.{scenario}.{case} is marked not_expressible but has a runtime observation")
                elif classification["kind"] == "comparable":
                    raise SystemExit(f"{client}.{scenario}.{case} is marked comparable but has no observation")
        elif entry is not None:
            raise SystemExit(f"{client}.{scenario} must include an observation and its explicit capability classification")
        elif isinstance(value, dict) and value.get("kind") == "not_expressible":
            raise SystemExit(f"{client}.{scenario} claims not_expressible without a checked contract entry")
    return observed


def comparable_observation(
    value: Any,
    contract: dict[str, dict[str, dict[str, dict[str, str]]]],
    scenario: str,
    client: str,
    other_client: str,
) -> Any:
    if isinstance(value, dict) and set(value) == {"observation", "capabilities"}:
        observation = dict(value["observation"])
        client_contract = contract.get(scenario, {})
        for case in set().union(*(set(classifications) for classifications in client_contract.values())) if client_contract else ():
            if (
                client_contract[client][case]["kind"] != "comparable"
                or client_contract[other_client][case]["kind"] != "comparable"
            ):
                observation.pop(case, None)
        return observation
    return value


def float_semantic_paths_for_scenario(
    scenario: str,
    inputs: dict[str, Any],
    schema: dict[str, Any],
) -> frozenset[str]:
    """Return schema-derived floating paths in shared typed-query intents."""
    intent = inputs.get(scenario, {}).get("query", {})
    kind = intent.get("kind")
    field = intent.get("field")
    field_type = schema.get("fields", {}).get(field, {}).get("type")
    if kind == "range" and field_type == "float64":
        return frozenset({"$.intent.lower", "$.intent.upper"})
    if kind == "geo_location":
        return frozenset({"$.intent.latitude", "$.intent.longitude"})
    if kind == "geo_box":
        return frozenset({"$.intent.south", "$.intent.west", "$.intent.north", "$.intent.east"})
    return frozenset()


def main() -> int:
    corpus = json.loads(CORPUS.read_text())
    scenarios, capabilities = scenario_contract(corpus)
    scenario_inputs = validate_shared_typed_inputs(corpus, scenarios)

    with tempfile.TemporaryDirectory(prefix="searchkernel-real-e2e-parity-") as temporary:
        output_dir = Path(temporary)
        base_env = os.environ.copy()
        commands = {
            "rust": ["bash", "-c", "source tools/client-e2e-runtime.sh && prepare_client_e2e_runtime && cargo run --quiet -p searchkernel --features native --example try_searchkernel_client_api_e2e"],
            "python": ["bash", "tools/run-python-native-e2e.sh", "--include-facade"],
            "typescript": ["bash", "clients/typescript/scripts/run-real-e2e.sh"],
            "php": ["bash", "clients/php/examples/run_searchkernel_client_api_e2e.sh"],
            "ruby": ["bash", "clients/ruby/examples/run_searchkernel_client_api_e2e.sh"],
        }
        traces: dict[str, dict[str, Any]] = {}
        for client in CLIENTS:
            output = output_dir / f"{client}.json"
            env = base_env.copy()
            env["SEARCHKERNEL_REAL_E2E_PARITY_OUTPUT"] = str(output)
            run(commands[client], env)
            try:
                trace = json.loads(output.read_text())
            except (OSError, json.JSONDecodeError) as error:
                raise SystemExit(f"{client} did not produce a valid real-engine parity snapshot: {error}") from error
            observations = scenario_observations(client, trace, scenarios, capabilities)
            validate_observed_intents(client, observations, scenario_inputs)
            traces[client] = observations

        failed = False
        for scenario in scenarios:
            baseline_client = CLIENTS[0]
            for client in CLIENTS[1:]:
                baseline = comparable_observation(traces[baseline_client][scenario], capabilities, scenario, baseline_client, client)
                actual = comparable_observation(traces[client][scenario], capabilities, scenario, client, baseline_client)
                differences = compare(
                    baseline,
                    actual,
                    float_semantic_paths=float_semantic_paths_for_scenario(
                        scenario, scenario_inputs, corpus.get("schema", {})
                    ),
                )
                if differences:
                    failed = True
                    print(f"{scenario}: {baseline_client} != {client}")
                    for difference in differences:
                        print(f"  {difference}")
                    print(json.dumps({baseline_client: baseline, client: actual}, indent=2, sort_keys=True))
        if failed:
            return 1
    classifications = sum(len(cases) for clients in capabilities.values() for cases in clients.values())
    print(f"real-engine cross-client parity PASS ({len(CLIENTS)} clients, {len(scenarios)} required scenarios; {classifications} checked per-client case classifications; float tolerance {FLOAT_TOLERANCE:g})")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
