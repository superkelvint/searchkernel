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


def main() -> int:
    corpus = json.loads(CORPUS.read_text())
    scenarios = corpus.get("parity_scenarios")
    if not isinstance(scenarios, list) or not scenarios or any(not isinstance(item, str) or not item for item in scenarios):
        raise SystemExit("parity_scenarios must contain non-empty scenario IDs")
    if len(scenarios) != len(set(scenarios)):
        raise SystemExit("parity scenario IDs must be unique")

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
            if trace.get("version") != 1 or not isinstance(trace.get("scenarios"), dict):
                raise SystemExit(f"{client} snapshot has an invalid version or scenarios object")
            missing = sorted(set(scenarios) - trace["scenarios"].keys())
            if missing:
                raise SystemExit(f"{client} snapshot is missing scenarios: {', '.join(missing)}")
            traces[client] = trace["scenarios"]

        failed = False
        for scenario in scenarios:
            baseline_client = CLIENTS[0]
            baseline = traces[baseline_client][scenario]
            for client in CLIENTS[1:]:
                differences = compare(baseline, traces[client][scenario])
                if differences:
                    failed = True
                    print(f"{scenario}: {baseline_client} != {client}")
                    for difference in differences:
                        print(f"  {difference}")
                    print(json.dumps({baseline_client: baseline, client: traces[client][scenario]}, indent=2, sort_keys=True))
        if failed:
            return 1
    print(f"real-engine cross-client parity PASS ({len(CLIENTS)} clients, {len(scenarios)} shared scenarios; float tolerance {FLOAT_TOLERANCE:g})")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
