#!/usr/bin/env python3
import importlib.util
import unittest
from pathlib import Path

P = Path(__file__).with_name("trusted_pr_authority.py")
S = importlib.util.spec_from_file_location("trusted_pr_authority", P)
M = importlib.util.module_from_spec(S); S.loader.exec_module(M)

class TrustedAuthorityTests(unittest.TestCase):
    def test_candidate_cannot_redefine_authorization_executor(self):
        for path in (".github/workflows/behavioral-verification.yml", ".github/workflows/ci-image-runner.yml", ".github/ci/exact_head.py", "verification/dag.toml", "dev", "reference/vespa/m14/verify-m14.sh", "tools/check-layout.sh", ".moon/tasks.yml"):
            with self.subTest(path=path): self.assertTrue(M.protected(path))

    def test_product_code_is_not_authority(self):
        for path in ("core/crates/searchkernel/src/lib.rs", "clients/python/searchkernel/client.py", "searchkerneld/src/main.rs"):
            with self.subTest(path=path): self.assertFalse(M.protected(path))

if __name__ == "__main__": unittest.main()
