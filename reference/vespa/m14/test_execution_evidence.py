#!/usr/bin/env python3
"""Regression tests for the M14 native HNSW execution evidence gate."""

import unittest

from validate_execution_evidence import (
    EvidenceError,
    validate_execution_evidence,
    validate_scenario,
)


def ann_evidence(*, approximate: bool = True, algorithm: str = "index top k") -> dict:
    return {
        "queries": [
            {
                "stage": "ann",
                "wanted_approximate": approximate,
                "has_index": True,
                "algorithm": algorithm,
                "explore_additional_hits": 2,
                "distance_threshold": 0.75,
                "approximate_nns_searches_performed": 1 if approximate else 0,
                "approximate_nns_distances_computed": 8 if approximate else 0,
                "approximate_nns_nodes_visited": 8 if approximate else 0,
                "exact_nns_distances_computed": 0 if approximate else 8,
            },
            {
                "stage": "reopen-ann",
                "wanted_approximate": approximate,
                "has_index": True,
                "algorithm": algorithm,
                "explore_additional_hits": 2,
                "distance_threshold": 0.75,
                "approximate_nns_searches_performed": 1 if approximate else 0,
                "approximate_nns_distances_computed": 8 if approximate else 0,
                "approximate_nns_nodes_visited": 8 if approximate else 0,
                "exact_nns_distances_computed": 0 if approximate else 8,
            },
        ]
    }


def scenario_query(
    stage: str, *, approximate: bool, explore: int, threshold: float | None = None
) -> dict:
    return {
        "id": stage,
        "trace": {
            "blueprint": {
                "wanted_approximate": approximate,
                "has_index": True,
                "algorithm": "index top k" if approximate else "exact",
                "explore_additional_hits": explore,
                "distance_threshold": threshold,
            },
            "setup": {
                "tag": "query_setup_stats",
                "stats": {
                    "approximate_nns_searches_performed": 1 if approximate else 0,
                    "approximate_nns_distances_computed": 8 if approximate else 0,
                    "approximate_nns_nodes_visited": 8 if approximate else 0,
                },
            },
            "execution": {
                "tag": "query_execution_stats",
                "stats": {"exact_nns_distances_computed": 0 if approximate else 8},
            },
        },
    }


def query_control_scenario() -> tuple[dict, dict]:
    scenario = {
        "id": "hnsw-query-controls",
        "steps": [
            {"id": "exact", "approximate": False, "explore_additional_hits": 0},
            {
                "id": "ann",
                "approximate": True,
                "explore_additional_hits": 2,
                "distance_threshold": 0.75,
            },
            {
                "id": "ann-extra-explore",
                "approximate": True,
                "explore_additional_hits": 5,
                "distance_threshold": 0.75,
            },
        ],
    }
    raw = {
        "evidence": [
            scenario_query("exact", approximate=False, explore=0),
            scenario_query("ann", approximate=True, explore=2, threshold=0.75),
            scenario_query(
                "ann-extra-explore", approximate=True, explore=0, threshold=0.75
            ),
            scenario_query("threshold-probe", approximate=True, explore=1, threshold=0.01)
            | {"hit_count": 0},
        ]
    }
    return scenario, raw


class ExecutionEvidenceTest(unittest.TestCase):
    def test_real_ann_evidence_is_accepted(self) -> None:
        validate_execution_evidence(ann_evidence())

    def test_exact_control_is_accepted(self) -> None:
        evidence = ann_evidence(approximate=False, algorithm="exact")
        validate_execution_evidence(evidence, require_ann=False)

    def test_exact_substitution_for_ann_is_rejected(self) -> None:
        evidence = ann_evidence(approximate=False, algorithm="exact")
        with self.assertRaises(EvidenceError):
            validate_execution_evidence(evidence)

    def test_exact_fallback_cannot_claim_ann_execution(self) -> None:
        evidence = ann_evidence(algorithm="exact fallback")
        with self.assertRaises(EvidenceError):
            validate_execution_evidence(evidence)

    def test_missing_index_cannot_claim_ann_execution(self) -> None:
        evidence = ann_evidence()
        evidence["queries"][0]["has_index"] = False
        with self.assertRaises(EvidenceError):
            validate_execution_evidence(evidence)

    def test_zero_approximate_searches_cannot_claim_ann_execution(self) -> None:
        evidence = ann_evidence()
        evidence["queries"][0]["approximate_nns_searches_performed"] = 0
        with self.assertRaises(EvidenceError):
            validate_execution_evidence(evidence)

    def test_changed_distance_threshold_is_rejected_for_its_stage(self) -> None:
        scenario, raw = query_control_scenario()
        raw["evidence"][1]["trace"]["blueprint"]["distance_threshold"] = 0.5
        with self.assertRaisesRegex(EvidenceError, "ann"):
            validate_scenario(scenario, raw)

    def test_changed_explore_control_is_rejected_for_its_stage(self) -> None:
        scenario, raw = query_control_scenario()
        with self.assertRaisesRegex(EvidenceError, "ann-extra-explore"):
            validate_scenario(scenario, raw)


if __name__ == "__main__":
    unittest.main()
