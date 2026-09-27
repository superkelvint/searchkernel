#!/usr/bin/env python3
import unittest
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1]
class QualificationPolicyTests(unittest.TestCase):
    def test_gate_is_wired_to_release_artifacts(self):
        workflow=(ROOT/'.github/workflows/ci-image-runner.yml').read_text()
        self.assertIn('tools/qualify-consumer-release.py /tmp/searchkernel-consumer-release',workflow)
        self.assertLess(workflow.index('tools/package-consumer-release.py --output /tmp/searchkernel-consumer-release'),workflow.index('tools/qualify-consumer-release.py /tmp/searchkernel-consumer-release'))
    def test_gate_has_no_workspace_client_imports_or_binary_overrides(self):
        gate=(ROOT/'tools/qualify-consumer-release.py').read_text()
        self.assertNotIn('clients/python',gate); self.assertNotIn('clients/typescript',gate)
        self.assertNotIn('target/debug/searchkerneld',gate); self.assertNotIn('SK_NATIVE_SDK',gate)
    def test_all_supported_rows_own_same_packaged_gate(self):
        import json
        authority=json.loads((ROOT/'docs/release-support.json').read_text())
        self.assertTrue(authority['supported'])
        self.assertEqual({'tools/qualify-consumer-release.py dist/release'},{r.get('qualification') for r in authority['supported']})
if __name__=='__main__': unittest.main()
