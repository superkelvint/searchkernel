"""Structural enforcement for generated Portable API model constructors."""

import dataclasses
import inspect
import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parents[1]))

from searchkernel.generated import client as generated  # noqa: E402


def _generated_dataclasses() -> list[type]:
    classes = {
        value
        for value in vars(generated).values()
        if isinstance(value, type)
        and value.__module__ == generated.__name__
        and dataclasses.is_dataclass(value)
    }
    return sorted(classes, key=lambda value: value.__name__)


class GeneratedKeywordOnlyTest(unittest.TestCase):
    def test_every_generated_dataclass_has_only_keyword_parameters(self):
        models = _generated_dataclasses()
        self.assertGreaterEqual(len(models), 4)
        for model in models:
            with self.subTest(model=model.__name__):
                signature = inspect.signature(model)
                self.assertTrue(
                    all(
                        parameter.kind is inspect.Parameter.KEYWORD_ONLY
                        for parameter in signature.parameters.values()
                    ),
                    signature,
                )

    def test_representative_models_reject_positional_construction(self):
        document = generated.Document(id="1", fields={})
        cases = (
            (generated.Document, ("1", {})),
            (generated.UpsertRequest, (document,)),
            (generated.SearchRequest, ("query",)),
            (generated.IndexDefinition, ("products", [])),
        )
        for model, args in cases:
            with self.subTest(model=model.__name__):
                with self.assertRaises(TypeError):
                    model(*args)

        self.assertEqual(generated.Document(id="1", fields={}).id, "1")
        self.assertEqual(generated.UpsertRequest(document=document).document, document)
        self.assertEqual(generated.SearchRequest(query="query").query, "query")
        self.assertEqual(generated.IndexDefinition(name="products", fields=[]).name, "products")


if __name__ == "__main__":
    unittest.main()
