#!/usr/bin/env python3
"""Run the full real-engine showcases and compare shared semantic observations."""
from __future__ import annotations

import json
import math
import os
from pathlib import Path
import subprocess
import sys
import tempfile
from typing import Any

ROOT = Path(__file__).resolve().parents[1]
CORPUS = ROOT / "reference/client-real-e2e-parity/corpus.json"
CLIENTS = ("rust", "python", "typescript", "php", "ruby")
FLOAT_TOLERANCE = 1e-6
INTEGER_FIELDS = {
    "total", "total_hits", "count", "products", "rating", "inventory",
    "rerank_count", "hits", "limit", "offset", "integer_value", "value",
    "documents", "results", "results_full", "nodes",
}


def compare(expected: Any, actual: Any, path: str = "$", differences: list[str] | None = None) -> list[str]:
    """Collect structural differences, allowing only small finite float drift."""
    if differences is None:
        differences = []
    if isinstance(expected, dict) and isinstance(actual, dict):
        if expected.keys() != actual.keys():
            differences.append(f"{path}: keys differ: {sorted(expected)} != {sorted(actual)}")
            return differences
        for key in sorted(expected):
            compare(expected[key], actual[key], f"{path}.{key}", differences)
        return differences
    if isinstance(expected, list) and isinstance(actual, list):
        if len(expected) != len(actual):
            differences.append(f"{path}: lengths differ: {len(expected)} != {len(actual)}")
            return differences
        for index, (left, right) in enumerate(zip(expected, actual)):
            compare(left, right, f"{path}[{index}]", differences)
        return differences
    if type(expected) is bool or type(actual) is bool:
        if type(expected) is not type(actual) or expected != actual:
            differences.append(f"{path}: {expected!r} != {actual!r}")
        return differences
    if isinstance(expected, (int, float)) and isinstance(actual, (int, float)):
        if path.rsplit(".", 1)[-1] in INTEGER_FIELDS:
            if type(expected) is not int or type(actual) is not int or expected != actual:
                differences.append(f"{path}: integer semantics require exact integers, got {expected!r} != {actual!r}")
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
                elif kind != "runtime_rejected":
                    raise SystemExit(f"{scenario}.{client}.{case}: unsupported case classification")
    return scenarios, capabilities


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
                    raise SystemExit(f"{client}.{scenario}.{case} is marked not_expressible but has a runtime observation")
        elif entry is not None:
            raise SystemExit(f"{client}.{scenario} must include an observation and its explicit capability classification")
        elif isinstance(value, dict) and value.get("kind") == "not_expressible":
            raise SystemExit(f"{client}.{scenario} claims not_expressible without a checked contract entry")
    return observed


def comparable_observation(value: Any, contract: dict[str, dict[str, dict[str, dict[str, str]]]], scenario: str) -> Any:
    if isinstance(value, dict) and set(value) == {"observation", "capabilities"}:
        observation = dict(value["observation"])
        client_contract = contract.get(scenario, {})
        for case in set().union(*(set(classifications) for classifications in client_contract.values())) if client_contract else ():
            if any(
                client_contract[client][case]["kind"] == "not_expressible"
                for client in client_contract
            ):
                observation.pop(case, None)
        return observation
    return value


def main() -> int:
    corpus = json.loads(CORPUS.read_text())
    scenarios, capabilities = scenario_contract(corpus)

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
            traces[client] = scenario_observations(client, trace, scenarios, capabilities)

        failed = False
        for scenario in scenarios:
            baseline_client = CLIENTS[0]
            baseline = comparable_observation(traces[baseline_client][scenario], capabilities, scenario)
            for client in CLIENTS[1:]:
                actual = comparable_observation(traces[client][scenario], capabilities, scenario)
                differences = compare(baseline, actual)
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
