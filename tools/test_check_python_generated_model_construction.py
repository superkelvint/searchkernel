"""Tests for the handwritten/generated-model construction boundary checker."""

import importlib.util
import tempfile
import unittest
from pathlib import Path


_ROOT = Path(__file__).resolve().parents[1]
_CHECKER_PATH = _ROOT / "tools/check-python-generated-model-construction.py"
_SPEC = importlib.util.spec_from_file_location("generated_model_checker", _CHECKER_PATH)
assert _SPEC and _SPEC.loader
checker = importlib.util.module_from_spec(_SPEC)
_SPEC.loader.exec_module(checker)


class GeneratedModelCheckerTest(unittest.TestCase):
    def _violations(self, source: str):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "handwritten.py"
            path.write_text(source, encoding="utf-8")
            names = checker._generated_model_names(
                _ROOT / "clients/python/searchkernel/generated/models.py"
            )
            return checker._analyze_file(path, names)

    def test_rejects_positional_generated_model_construction(self):
        violations = self._violations(
            "from .generated.models import Document, UpsertRequest\n"
            "Document(identifier, fields)\n"
            "UpsertRequest(document)\n"
        )
        self.assertEqual([(line, target) for _, line, target in violations], [
            (2, "Document"),
            (3, "UpsertRequest"),
        ])

    def test_allows_keyword_construction_and_local_shadowing(self):
        self.assertEqual(
            self._violations(
                "from .generated.models import Document\n"
                "Document(id=identifier, fields=fields)\n"
            ),
            [],
        )
        self.assertEqual(
            self._violations(
                "from .generated.models import Document\n"
                "class Document:\n"
                "    pass\n"
                "Document(identifier, fields)\n"
            ),
            [],
        )


if __name__ == "__main__":
    unittest.main()
