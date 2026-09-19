"""The Python protocol path is the flatc object API, not a second model."""

from __future__ import annotations

import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parents[1]))

from searchkernel import Group, agg, g  # noqa: E402
from searchkernel.client import value  # noqa: E402
from searchkernel.protocol import pack_object, pack_request  # noqa: E402
from SearchKernel.Protocol.V2.RequestEnvelope import RequestEnvelopeT  # noqa: E402
from SearchKernel.V2.Document import DocumentT  # noqa: E402
from SearchKernel.V2.InsertRequest import InsertRequestT  # noqa: E402
from SearchKernel.V2.TensorValue import TensorValueT  # noqa: E402
from SearchKernel.V2.TensorPayloadKind import TensorPayloadKind  # noqa: E402
from SearchKernel.V2.DenseTensorValues import DenseTensorValuesT  # noqa: E402
from SearchKernel.V2.IndexDefinition import IndexDefinitionT  # noqa: E402


class FlatBuffersObjectApiTest(unittest.TestCase):
    def test_recursive_value_and_envelope_round_trip(self):
        document = DocumentT(
            id="doc:1",
            fields=[],
        )
        document.fields = [
            __import__("SearchKernel.V2.ValueEntry", fromlist=["ValueEntryT"]).ValueEntryT(
                key="payload", value=value({"nested": [1, True]})
            )
        ]
        raw = pack_request("insert", InsertRequestT(document=document))
        envelope = RequestEnvelopeT.InitFromPackedBuf(raw)
        self.assertEqual(envelope.payload.document.id, "doc:1")
        nested = envelope.payload.document.fields[0].value.kind.fields[0].value.kind.values
        self.assertEqual([item.kind.value for item in nested], [1, True])

    def test_widened_v2_objects_pack_without_schema_mapper(self):
        tensor = TensorValueT(
            typeSpec="tensor<float>(x[2])",
            payloadType=TensorPayloadKind.DenseTensorValues,
            payload=DenseTensorValuesT(values=[0.25, 0.75]),
        )
        self.assertIsInstance(tensor, TensorValueT)

        grouping = Group.by("category").output(agg.count().as_("count"))
        request = grouping.to_request()
        raw = pack_object(request)
        self.assertGreater(len(raw), 0)

        expression = (g.field("price") + 1).model
        self.assertEqual(expression.expr.left.expr.field, "price")
        self.assertEqual(expression.expr.right.expr.value.kind.value, 1)


if __name__ == "__main__":
    unittest.main()
