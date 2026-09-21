#!/usr/bin/env python3
"""Regression tests for the M14 native HNSW execution evidence gate."""

import unittest

from validate_execution_evidence import EvidenceError, validate_execution_evidence


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


if __name__ == "__main__":
    unittest.main()
