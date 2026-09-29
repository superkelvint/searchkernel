import importlib.util
from pathlib import Path
import unittest

SPEC = importlib.util.spec_from_file_location(
    "real_e2e_parity", Path(__file__).with_name("check-client-real-e2e-parity.py")
)
assert SPEC is not None and SPEC.loader is not None
PARITY = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(PARITY)
compare = PARITY.compare


class RealE2EParityComparatorTests(unittest.TestCase):
    def test_equal_structures_pass(self):
        self.assertEqual([], compare({"ids": ["a", "b"], "count": 2}, {"ids": ["a", "b"], "count": 2}))

    def test_float_tolerance_is_relative_and_absolute(self):
        self.assertEqual([], compare({"score": 100.0}, {"score": 100.00005}))
        self.assertTrue(compare({"score": 100.0}, {"score": 100.01}))

    def test_integer_and_boolean_kinds_remain_exact(self):
        self.assertTrue(compare({"count": 1}, {"count": 2}))
        self.assertTrue(compare({"count": 1}, {"count": True}))

    def test_missing_fields_and_ordered_ids_fail(self):
        self.assertTrue(compare({"ids": ["a", "b"]}, {"ids": ["a"]}))
        self.assertTrue(compare({"ids": ["a", "b"]}, {"ids": ["b", "a"]}))


if __name__ == "__main__":
    unittest.main()
