"""The retained Python package must import without retired client exports."""

import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parents[1]))


class PackageImportTest(unittest.TestCase):
    def test_all_public_exports_resolve(self):
        import searchkernel

        self.assertTrue(searchkernel.__all__)
        for name in searchkernel.__all__:
            with self.subTest(name=name):
                self.assertTrue(hasattr(searchkernel, name))


if __name__ == "__main__":
    unittest.main()
