import json
import os
import unittest
from pathlib import Path

from searchkernel.protocol import unpack_response
from SearchKernel.V2.SearchResult import SearchResultT
from searchkernel.ergonomic import SearchResults


@unittest.skipUnless(os.getenv("SEARCHKERNEL_RESULT_CASES"), "run through ./dev verifier")
class ResultResponseConformance(unittest.TestCase):
    def test_production_response_bytes(self):
        cases = json.loads(Path(os.environ["SEARCHKERNEL_RESULT_CASES"]).read_text())
        case_directory = Path(os.environ["SEARCHKERNEL_RESULT_CASE_DIR"])
        traces = {}
        for case in cases["valid"]:
            with self.subTest(scenario=case["id"]):
                encoded = (case_directory / case["file"]).read_bytes()
                envelope = unpack_response(encoded, "search")
                self.assertIsInstance(envelope.payload, SearchResultT)
                result = SearchResults(envelope.payload)
                self.assertIsNotNone(result.root)
                normalized = json.loads((case_directory / case["json"]).read_text())
                self.assertIn("payload", normalized)
                traces[case["id"]] = (case_directory / case["json"]).read_text()

        rejected = []
        for case in cases["malformed"]:
            with self.subTest(malformed=case["id"]):
                encoded = (Path(os.environ["SEARCHKERNEL_RESULT_FIXTURE_DIR"]) / case["file"]).read_bytes()
                with self.assertRaises((ValueError, TypeError, IndexError)):
                    envelope = unpack_response(encoded, "search")
                    SearchResults(envelope.payload)
                rejected.append(case["id"])

        traces["malformed_rejected"] = rejected
        Path(os.environ["SEARCHKERNEL_RESULT_TRACE_DIR"], "python.json").write_text(
            json.dumps(traces, sort_keys=True)
        )
