#!/usr/bin/env python3
"""Regression tests for native include ownership checks."""

import pathlib
import subprocess
import tempfile
import unittest


ROOT = pathlib.Path(__file__).resolve().parents[1]
CHECKER = ROOT / "tools/check-native-include-boundary.py"


class NativeIncludeBoundaryTests(unittest.TestCase):
    def test_generated_consumer_bundle_is_accepted(self):
        result = subprocess.run(
            ["python3", str(CHECKER), "--root", str(ROOT)],
            check=False,
            capture_output=True,
            text=True,
        )
        self.assertEqual(result.returncode, 0, result.stderr)

    def test_missing_generated_consumer_bundle_fails_closed(self):
        with tempfile.TemporaryDirectory(prefix="searchkernel-native-boundary-") as tmp:
            result = subprocess.run(
                ["python3", str(CHECKER), "--root", tmp],
                check=False,
                capture_output=True,
                text=True,
            )

        self.assertNotEqual(result.returncode, 0)
        self.assertIn("generated consumer bundle is missing", result.stderr)

    def test_handwritten_vespa_include_outside_adapter_is_rejected(self):
        with tempfile.TemporaryDirectory(prefix="searchkernel-native-boundary-") as tmp:
            root = pathlib.Path(tmp)
            source = root / "core/crates/searchkernel/src/unexpected.rs"
            source.parent.mkdir(parents=True)
            source.write_text("#include <vespa/document/document.h>\n", encoding="utf-8")

            result = subprocess.run(
                ["python3", str(CHECKER), "--root", str(root)],
                check=False,
                capture_output=True,
                text=True,
            )

        self.assertNotEqual(result.returncode, 0)
        self.assertIn("unexpected.rs", result.stderr)
        self.assertIn("Vespa C++ headers leaked outside native adapter", result.stderr)


if __name__ == "__main__":
    unittest.main()
