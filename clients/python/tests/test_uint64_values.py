import unittest

from searchkernel import uint64
from searchkernel.client import value
from searchkernel.protocol import pack_object
from searchkernel._semantic import is_semantic_table
from searchkernel._wire.v1.generated.SearchKernel.Wire.UInt64Value import UInt64ValueT
from searchkernel._wire.v1.generated.SearchKernel.Wire.Value import ValueT
from searchkernel._wire.v1.generated.SearchKernel.Wire.ValueKind import ValueKind


class UInt64ValueTest(unittest.TestCase):
    def test_plain_integer_keeps_the_existing_signed_variant(self):
        self.assertEqual(value(2**63 - 1).kindType, ValueKind.Int64Value)

    def test_uint64_preserves_variant_at_boundaries(self):
        for number in (0, 2**63 - 1, 2**63, 2**64 - 1):
            with self.subTest(number=number):
                encoded = value(uint64(number))
                self.assertEqual(encoded.kindType, ValueKind.UInt64Value)
                self.assertTrue(is_semantic_table(encoded.kind, "UInt64Value"))
                self.assertEqual(encoded.kind.value, number)

                decoded = ValueT.InitFromPackedBuf(pack_object(encoded))
                self.assertEqual(decoded.kindType, ValueKind.UInt64Value)
                self.assertEqual(decoded.kind.value, number)

    def test_uint64_rejects_negative_overflow_and_non_integer_values(self):
        for invalid in (-1, 2**64, True, 1.0, "1"):
            with self.subTest(invalid=invalid):
                with self.assertRaises((TypeError, ValueError)):
                    uint64(invalid)


if __name__ == "__main__":
    unittest.main()
