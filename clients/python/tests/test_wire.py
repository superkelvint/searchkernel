from __future__ import annotations

import sys
import unittest
from pathlib import Path

import flatbuffers

sys.path.insert(0, str(Path(__file__).parents[1]))

from searchkernel import generated  # noqa: E402
from searchkernel.wire import PortableResponseError, decode_response, encode_request  # noqa: E402
from SearchKernel.Protocol.V2 import RequestEnvelope, ResponseEnvelope, ResponsePayload  # noqa: E402
from SearchKernel.V2 import DeleteRequest, Error, ErrorDetail, SearchResult, TotalHits, UpsertRequest  # noqa: E402


class WireEncodingTest(unittest.TestCase):
    def test_populated_unsupported_search_fields_fail_closed(self):
        cases = (
            ("ranking", generated.RankingRequest(profile="rank")),
            ("inputs", [generated.QueryInput(name="embedding")]),
            (
                "filter sources",
                generated.FilterRequest(sources=["default"]),
            ),
            (
                "filter parameters",
                generated.FilterRequest(parameters={"min": generated.Value.int64(1)}),
            ),
            (
                "query model",
                generated.QueryModel(query_string="hello"),
            ),
            (
                "presentation",
                generated.PresentationRequest(summary="short"),
            ),
            ("properties", {"trace": generated.Value.string("on")}),
            ("query profile", "profile"),
            ("search chain", "chain"),
        )
        for field, unsupported in cases:
            with self.subTest(field=field):
                request = generated.SearchRequest()
                if field == "filter sources" or field == "filter parameters":
                    request.filters = unsupported
                else:
                    setattr(request, {
                        "query model": "query_model",
                        "query profile": "query_profile",
                        "search chain": "search_chain",
                    }.get(field, field), unsupported)
                with self.assertRaisesRegex(ValueError, "unsupported populated field"):
                    encode_request("search", request)

    def test_populated_unsupported_nested_request_fields_fail_closed(self):
        cases = (
            generated.SearchRequest(
                query=generated.QuerySource(
                    kind="text",
                    text="hello",
                    parameters={"user": generated.Value.string("kelvin")},
                )
            ),
            generated.SearchRequest(
                sort=[
                    generated.SortExpr(
                        target=generated.SortTarget(kind="rank_feature", feature="score")
                    )
                ]
            ),
            generated.SearchRequest(
                sort=[
                    generated.SortExpr(
                        target=generated.SortTarget(kind="field", field="price"),
                        missing=generated.MissingPolicy(kind="first"),
                    )
                ]
            ),
            generated.SearchRequest(
                filters=generated.FilterRequest(
                    structured=[
                        generated.FilterExpr(
                            kind="comparison",
                            field=generated.QueryFieldRef(path="price"),
                            op="eq",
                            value=generated.Value(
                                kind="string", value="1", type_spec="tensor<float>(x{})"
                            ),
                        )
                    ]
                )
            ),
            generated.SearchRequest(
                grouping=[
                    generated.GroupingRequest(
                        source=generated.GroupingSourceKind(
                            kind="structured",
                            value=generated.GroupingAllOperation(
                                operations=[
                                    generated.GroupingOperation(
                                        kind="group",
                                        expression=generated.GroupingExpression(
                                            kind="attribute",
                                            field="category",
                                            value=generated.Value.string("unexpected"),
                                        ),
                                    )
                                ]
                            ),
                        )
                    )
                ]
            ),
        )
        for request in cases:
            with self.subTest(request=request):
                with self.assertRaisesRegex(ValueError, "unsupported populated field"):
                    encode_request("search", request)

    def test_unsupported_query_syntax_is_explicit(self):
        for kind in ("yql", "typed"):
            with self.subTest(kind=kind):
                request = generated.SearchRequest(query=generated.QuerySource(kind=kind))
                with self.assertRaisesRegex(ValueError, "unsupported query source"):
                    encode_request("search", request)

    def test_mutation_conditions_are_preserved(self):
        cases = (
            (
                "upsert",
                generated.UpsertRequest(
                    document=generated.Document(id="doc:1"), condition="price < 10"
                ),
                UpsertRequest.UpsertRequest,
            ),
            (
                "delete",
                generated.DeleteRequest(id="doc:1", condition="category == 'old'"),
                DeleteRequest.DeleteRequest,
            ),
        )
        for operation, request, table_type in cases:
            with self.subTest(operation=operation):
                envelope = RequestEnvelope.RequestEnvelope.GetRootAs(
                    encode_request(operation, request), 0
                )
                payload = envelope.Payload()
                decoded = table_type()
                decoded.Init(payload.Bytes, payload.Pos)
                self.assertEqual(decoded.Condition(), request.condition.encode())

    @staticmethod
    def _response(builder, payload_type, payload):
        ResponseEnvelope.Start(builder)
        ResponseEnvelope.AddProtocolVersion(builder, 2)
        ResponseEnvelope.AddPayloadType(builder, payload_type)
        ResponseEnvelope.AddPayload(builder, payload)
        envelope = ResponseEnvelope.End(builder)
        builder.Finish(envelope, file_identifier=b"SKRS")
        return bytes(builder.Output())

    def test_error_code_and_all_error_details_are_decoded(self):
        builder = flatbuffers.Builder(1024)
        detail_offsets = []
        for code, path, message in (("one", "a", "first"), ("two", "b", "second")):
            code_offset = builder.CreateString(code)
            path_offset = builder.CreateString(path)
            message_offset = builder.CreateString(message)
            ErrorDetail.Start(builder)
            ErrorDetail.AddCode(builder, code_offset)
            ErrorDetail.AddPath(builder, path_offset)
            ErrorDetail.AddMessage(builder, message_offset)
            detail_offsets.append(ErrorDetail.End(builder))
        builder.StartVector(4, len(detail_offsets), 4)
        for offset in reversed(detail_offsets):
            builder.PrependUOffsetTRelative(offset)
        details = builder.EndVector()
        message_offset = builder.CreateString("conflict")
        Error.Start(builder)
        Error.AddCode(builder, 5)
        Error.AddMessage(builder, message_offset)
        Error.AddDetails(builder, details)
        error = Error.End(builder)
        response = self._response(builder, ResponsePayload.ResponsePayload.SearchKernel_V2_Error, error)

        with self.assertRaises(PortableResponseError) as raised:
            decode_response(response, "search")
        self.assertEqual(raised.exception.error.code, "conflict")
        self.assertEqual([item.code for item in raised.exception.error.details], ["one", "two"])

    def test_total_hits_relation_is_symbolic(self):
        builder = flatbuffers.Builder(512)
        TotalHits.Start(builder)
        TotalHits.AddValue(builder, 3)
        TotalHits.AddRelation(builder, 1)
        total = TotalHits.End(builder)
        SearchResult.Start(builder)
        SearchResult.AddTotalHits(builder, total)
        result = SearchResult.End(builder)
        response = self._response(builder, ResponsePayload.ResponsePayload.SearchKernel_V2_SearchResult, result)

        self.assertEqual(decode_response(response, "search").total_hits.relation, "exact")


if __name__ == "__main__":
    unittest.main()
