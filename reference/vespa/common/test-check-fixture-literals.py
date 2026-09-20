#!/usr/bin/env python3
"""Regression tests for the fixture-literal production-source firewall."""

from __future__ import annotations

import importlib.util
from pathlib import Path
import unittest


MODULE_PATH = Path(__file__).with_name("check-fixture-literals.py")
SPEC = importlib.util.spec_from_file_location("check_fixture_literals", MODULE_PATH)
assert SPEC and SPEC.loader
MODULE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MODULE)


class FixtureLiteralCheckerTests(unittest.TestCase):
    def test_strips_plain_test_cfg(self):
        source = 'const production = "fixture";\n#[cfg(test)]\nmod tests { const value = "fixture"; }\n'
        self.assertEqual(MODULE.strip_rust_test_code(source), 'const production = "fixture";\n')

    def test_strips_test_cfg_with_native_feature(self):
        source = (
            'const production = "fixture";\n'
            '#[cfg(all(test, feature = "native"))]\n'
            'mod native_tests { const value = "fixture"; }\n'
        )
        self.assertEqual(MODULE.strip_rust_test_code(source), 'const production = "fixture";\n')

    def test_keeps_cfg_that_is_not_test_only(self):
        source = '#[cfg(any(test, feature = "native"))]\nconst value = "fixture";\n'
        self.assertEqual(MODULE.strip_rust_test_code(source), source)


if __name__ == "__main__":
    unittest.main()
