"""GET document fields must preserve the canonical ValueEntry map invariant."""

from __future__ import annotations

import unittest
from types import SimpleNamespace

from searchkernel import Index
from searchkernel.client import value
from searchkernel._wire.v1.generated.SearchKernel.Wire.Document import DocumentT
from searchkernel._wire.v1.generated.SearchKernel.Wire.ValueEntry import ValueEntryT


class _GetClient:
    def __init__(self, document: DocumentT):
        self.document = document

    def get(self, _identifier: str, _projection=None):
        return SimpleNamespace(document=self.document)


def _get(fields):
    return Index(_GetClient(DocumentT(id="doc:1", fields=fields)), ergonomic=True).get("doc:1")


class GetSemanticMapTest(unittest.TestCase):
    def test_get_rejects_duplicate_document_field_keys(self):
        with self.assertRaisesRegex(ValueError, "duplicate key"):
            _get([ValueEntryT("same", value(1)), ValueEntryT("same", value(2))])

    def test_get_rejects_null_document_field_entry(self):
        with self.assertRaisesRegex(ValueError, "malformed ValueEntry"):
            _get([None])

    def test_get_rejects_document_field_without_key(self):
        with self.assertRaisesRegex(ValueError, "malformed ValueEntry"):
            _get([ValueEntryT(None, value(1))])

    def test_get_rejects_document_field_without_value_payload(self):
        with self.assertRaisesRegex(ValueError, "missing Value payload"):
            _get([ValueEntryT("missing", None)])

    def test_get_preserves_explicit_null_and_valid_fields(self):
        self.assertEqual(
            _get([ValueEntryT("nothing", value(None)), ValueEntryT("count", value(3))]),
            {"_id": "doc:1", "nothing": None, "count": 3},
        )


if __name__ == "__main__":
    unittest.main()
