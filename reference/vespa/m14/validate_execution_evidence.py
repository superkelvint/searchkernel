#!/usr/bin/env python3
"""Validate that M14 queries selected Vespa's real HNSW execution path."""

from __future__ import annotations

import json
import sys
from typing import Any


class EvidenceError(ValueError):
    """Raised when runtime evidence cannot prove the requested execution path."""


def _number(query: dict[str, Any], name: str) -> float:
    value = query.get(name)
    if not isinstance(value, (int, float)) or isinstance(value, bool):
        raise EvidenceError(f"missing numeric evidence: {name}")
    return float(value)


def _validate_query(query: dict[str, Any], *, require_ann: bool) -> None:
    stage = query.get("stage", "")
    if not isinstance(stage, str) or not stage:
        raise EvidenceError("query evidence has no stage")
    if not isinstance(query.get("wanted_approximate"), bool):
        raise EvidenceError(f"{stage}: wanted_approximate is missing")
    if not isinstance(query.get("has_index"), bool):
        raise EvidenceError(f"{stage}: has_index is missing")
    algorithm = query.get("algorithm")
    if not isinstance(algorithm, str):
        raise EvidenceError(f"{stage}: algorithm is missing")

    approximate_searches = _number(query, "approximate_nns_searches_performed")
    approximate_distances = _number(query, "approximate_nns_distances_computed")
    approximate_nodes = _number(query, "approximate_nns_nodes_visited")
    exact_distances = _number(query, "exact_nns_distances_computed")

    if require_ann:
        if not query["wanted_approximate"]:
            raise EvidenceError(f"{stage}: approximate request selected exact execution")
        if not query["has_index"]:
            raise EvidenceError(f"{stage}: approximate request has no nearest-neighbor index")
        if algorithm not in ("index top k", "index top k using filter"):
            raise EvidenceError(f"{stage}: approximate request selected {algorithm!r}")
        if approximate_searches <= 0:
            raise EvidenceError(f"{stage}: no approximate nearest-neighbor search was performed")
        if approximate_distances <= 0 and approximate_nodes <= 0:
            raise EvidenceError(f"{stage}: ANN visited no nodes and computed no distances")
        if exact_distances != 0:
            raise EvidenceError(f"{stage}: ANN evidence includes exact distance evaluation")
    else:
        if query["wanted_approximate"]:
            raise EvidenceError(f"{stage}: exact control requested approximate execution")
        if algorithm not in ("exact", "exact fallback"):
            raise EvidenceError(f"{stage}: exact control selected {algorithm!r}")
        if approximate_searches != 0 or approximate_distances != 0 or approximate_nodes != 0:
            raise EvidenceError(f"{stage}: exact control performed ANN work")
        if exact_distances <= 0:
            raise EvidenceError(f"{stage}: exact control did not compute exact distances")


def _direct_queries(evidence: dict[str, Any]) -> list[dict[str, Any]]:
    queries = evidence.get("queries")
    if not isinstance(queries, list) or not all(isinstance(item, dict) for item in queries):
        raise EvidenceError("evidence does not contain query records")
    return queries


def validate_execution_evidence(
    evidence: dict[str, Any], *, require_ann: bool = True
) -> None:
    """Validate normalized query records used by unit tests and the verifier."""

    queries = _direct_queries(evidence)
    if not queries:
        raise EvidenceError("evidence contains no query records")
    for query in queries:
        _validate_query(query, require_ann=require_ann)
    if require_ann:
        explore_values = {_number(query, "explore_additional_hits") for query in queries}
        if max(explore_values) <= 0:
            raise EvidenceError("ANN evidence never exposed exploreAdditionalHits")
        thresholds = {_number(query, "distance_threshold") for query in queries}
        if not any(value < 100.0 for value in thresholds):
            raise EvidenceError("ANN evidence never exposed an effective discriminating threshold")


def _find_blueprints(node: Any) -> list[dict[str, Any]]:
    found: list[dict[str, Any]] = []
    if isinstance(node, dict):
        required = {"wanted_approximate", "has_index", "algorithm"}
        if required.issubset(node):
            found.append(node)
        for value in node.values():
            found.extend(_find_blueprints(value))
    elif isinstance(node, list):
        for value in node:
            found.extend(_find_blueprints(value))
    return found


def _find_stats(node: Any, tag: str) -> dict[str, Any]:
    if isinstance(node, dict):
        if node.get("tag") == tag and isinstance(node.get("stats"), dict):
            return node["stats"]
        for value in node.values():
            found = _find_stats(value, tag)
            if found:
                return found
    elif isinstance(node, list):
        for value in node:
            found = _find_stats(value, tag)
            if found:
                return found
    return {}


def _trace_record(record: dict[str, Any]) -> dict[str, Any]:
    traces = _find_blueprints(record.get("trace"))
    if len(traces) != 1:
        raise EvidenceError(
            f"{record.get('id', '<unknown>')}: expected one NN blueprint, found {len(traces)}"
        )
    blueprint = traces[0]
    setup_stats = _find_stats(record.get("trace"), "query_setup_stats")
    eval_stats = _find_stats(record.get("trace"), "query_execution_stats")
    return {
        "stage": record.get("id"),
        "wanted_approximate": blueprint.get("wanted_approximate"),
        "has_index": blueprint.get("has_index"),
        "algorithm": blueprint.get("algorithm"),
        "explore_additional_hits": blueprint.get("explore_additional_hits"),
        "distance_threshold": blueprint.get("distance_threshold"),
        "approximate_nns_searches_performed": setup_stats.get(
            "approximate_nns_searches_performed", 0
        ),
        "approximate_nns_distances_computed": setup_stats.get(
            "approximate_nns_distances_computed", 0
        ),
        "approximate_nns_nodes_visited": setup_stats.get("approximate_nns_nodes_visited", 0),
        "exact_nns_distances_computed": eval_stats.get("exact_nns_distances_computed", 0),
        "hit_count": record.get("hit_count"),
    }


def validate_scenario(scenario: dict[str, Any], raw: dict[str, Any]) -> None:
    """Validate raw driver evidence against the scenario's query controls."""

    records = raw.get("evidence")
    if not isinstance(records, list) or not records:
        raise EvidenceError("driver output contains no execution evidence")
    normalized = [_trace_record(record) for record in records if isinstance(record, dict)]
    if len(normalized) != len(records):
        raise EvidenceError("driver output contains malformed execution evidence")

    stages = [query["stage"] for query in normalized]
    if any(not isinstance(stage, str) or not stage for stage in stages):
        raise EvidenceError("driver output contains execution evidence without a stage")
    if len(set(stages)) != len(stages):
        duplicates = sorted({stage for stage in stages if stages.count(stage) > 1})
        raise EvidenceError(
            "driver output contains duplicate execution evidence stages: "
            + ", ".join(duplicates)
        )
    by_stage = {query["stage"]: query for query in normalized}
    if scenario.get("id") == "hnsw-query-controls":
        expected = [
            (
                step["id"],
                bool(step["approximate"]),
                step["explore_additional_hits"],
                step.get("distance_threshold"),
            )
            for step in scenario.get("steps", [])
        ]
    elif scenario.get("id") == "hnsw-mutation-reopen":
        expected = [
            (
                stage,
                bool(scenario["query_options"]["approximate"]),
                scenario["query_options"]["explore_additional_hits"],
                scenario["query_options"].get("distance_threshold"),
            )
            for stage in (
                "before_update",
                "after_update",
                "after_reopen",
                "after_delete",
                "after_delete_reopen",
            )
        ]
    else:
        raise EvidenceError(f"unsupported scenario: {scenario.get('id')}")

    expected_stages = {stage for stage, *_ in expected} | {"threshold-probe"}
    observed_stages = set(by_stage)
    if observed_stages != expected_stages:
        missing = sorted(expected_stages - observed_stages)
        unexpected = sorted(observed_stages - expected_stages)
        details = []
        if missing:
            details.append("missing: " + ", ".join(missing))
        if unexpected:
            details.append("unexpected: " + ", ".join(unexpected))
        raise EvidenceError("driver output stage mismatch (" + "; ".join(details) + ")")

    expected_queries: list[dict[str, Any]] = []
    for stage, approximate, expected_explore, expected_threshold in expected:
        query = by_stage.get(stage)
        if query is None:
            raise EvidenceError(f"scenario query {stage!r} has no execution evidence")
        _validate_query(query, require_ann=approximate)
        observed_explore = _number(query, "explore_additional_hits")
        if observed_explore != expected_explore:
            raise EvidenceError(
                f"{stage}: observed exploreAdditionalHits {observed_explore:g} "
                f"does not match requested value {expected_explore}"
            )
        if expected_threshold is not None:
            observed_threshold = _number(query, "distance_threshold")
            if observed_threshold != expected_threshold:
                raise EvidenceError(
                    f"{stage}: observed distanceThreshold {observed_threshold:g} "
                    f"does not match requested value {expected_threshold}"
                )
        expected_queries.append(query)

    ann_queries = [query for query in expected_queries if query["wanted_approximate"]]
    exact_queries = [query for query in expected_queries if not query["wanted_approximate"]]
    if exact_queries:
        validate_execution_evidence({"queries": exact_queries}, require_ann=False)
    if not ann_queries:
        raise EvidenceError("scenario contains no approximate query evidence")
    probe = by_stage.get("threshold-probe")
    if probe is None:
        raise EvidenceError("scenario has no threshold probe evidence")
    _validate_query(probe, require_ann=True)
    validate_execution_evidence({"queries": ann_queries + [probe]})
    if not any(query["explore_additional_hits"] > 0 for query in ann_queries):
        raise EvidenceError("scenario did not expose exploreAdditionalHits")
    if probe["hit_count"] != 0 or probe["distance_threshold"] >= 100.0:
        raise EvidenceError("scenario did not prove a threshold can exclude a hit")


def main() -> int:
    if len(sys.argv) != 3:
        raise SystemExit(f"usage: {sys.argv[0]} SCENARIO DRIVER_OUTPUT")
    with open(sys.argv[1], encoding="utf-8") as stream:
        scenario = json.load(stream)
    with open(sys.argv[2], encoding="utf-8") as stream:
        raw = json.load(stream)
    validate_scenario(scenario, raw)
    print("M14 execution evidence valid")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
