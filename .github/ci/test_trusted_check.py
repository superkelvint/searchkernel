#!/usr/bin/env python3
import importlib.util
import unittest
from pathlib import Path
from unittest.mock import patch

P = Path(__file__).with_name("trusted_check.py")
S = importlib.util.spec_from_file_location("trusted_check", P)
M = importlib.util.module_from_spec(S)
S.loader.exec_module(M)


class TrustedCheckTests(unittest.TestCase):
    def test_wrong_app_cannot_be_reused_as_trusted_check(self):
        calls = []

        def fake_request(token, method, url, payload=None):
            calls.append((method, url, payload))
            if method == "GET":
                return {
                    "check_runs": [
                        {
                            "id": 11,
                            "external_id": "trusted-pr:719:abc",
                            "app": {"id": 999},
                        }
                    ]
                }
            if method == "POST":
                return {"id": 22}
            self.fail(f"unexpected request: {method} {url}")

        with patch.object(M, "request", side_effect=fake_request):
            result = M.publish(
                token="token",
                api_url="https://api.example.invalid",
                repo="owner/repo",
                sha="abc",
                name="Trusted invariant gate",
                external_id="trusted-pr:719:abc",
                app_id=123,
                conclusion="success",
                summary="pass",
                details_url=None,
            )

        self.assertEqual(22, result["id"])
        self.assertEqual("GET", calls[0][0])
        self.assertEqual("POST", calls[1][0])
        self.assertEqual("abc", calls[1][2]["head_sha"])

    def test_matching_app_and_external_id_are_updated_in_place(self):
        calls = []

        def fake_request(token, method, url, payload=None):
            calls.append((method, url, payload))
            if method == "GET":
                return {
                    "check_runs": [
                        {
                            "id": 33,
                            "external_id": "trusted-pr:719:abc",
                            "app": {"id": 123},
                        }
                    ]
                }
            if method == "PATCH":
                return {"id": 33}
            self.fail(f"unexpected request: {method} {url}")

        with patch.object(M, "request", side_effect=fake_request):
            result = M.publish(
                token="token",
                api_url="https://api.example.invalid",
                repo="owner/repo",
                sha="abc",
                name="Trusted invariant gate",
                external_id="trusted-pr:719:abc",
                app_id=123,
                conclusion="failure",
                summary="fail closed",
                details_url="https://example.invalid/run",
            )

        self.assertEqual(33, result["id"])
        self.assertEqual("PATCH", calls[1][0])
        self.assertNotIn("head_sha", calls[1][2])
        self.assertEqual("failure", calls[1][2]["conclusion"])
        self.assertEqual("https://example.invalid/run", calls[1][2]["details_url"])


if __name__ == "__main__":
    unittest.main()
