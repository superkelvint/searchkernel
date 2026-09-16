"""C7 adapter for the production Python client and native transport."""

from __future__ import annotations

import sys
import json
from pathlib import Path


_PACKAGE_ROOT = Path(__file__).resolve().parents[1]
if str(_PACKAGE_ROOT) not in sys.path:
    sys.path.insert(0, str(_PACKAGE_ROOT))

from searchkernel import (  # noqa: E402
    Client,
    ClientError,
    DescribeRequest,
    Document,
    InsertRequest,
    NativeTransport,
    SearchRequest,
    request_from_json_text,
)
from searchkernel.generated.client import _read_response  # noqa: E402


def expected_abi_version() -> int:
    return 2


def open_transport(library_path: str, open_payload: bytes):
    return NativeTransport.open(library_path, open_payload)


def real_c5_smoke(library_path: str) -> None:
    with open_transport(library_path, b"recording") as transport:
        client = Client(transport)

        try:
            client.insert(InsertRequest(Document(id="c7-python-document")))
        except ClientError as error:
            if error.portable is None:
                raise AssertionError("mutation did not traverse generated error conversion") from error
        else:
            raise AssertionError("recording fixture unexpectedly executed mutation")

        try:
            client.search(SearchRequest())
        except ClientError as error:
            if error.portable is None:
                raise AssertionError("search did not traverse generated error conversion") from error
        else:
            raise AssertionError("recording fixture unexpectedly executed search")

        client.describe(DescribeRequest())

    with open_transport(library_path, b"scripted") as transport:
        try:
            Client(transport).search(SearchRequest())
        except ClientError as error:
            if error.portable is None or error.transport is not None:
                raise AssertionError("scripted Portable API error was not mapped by the client") from error
        else:
            raise AssertionError("scripted Portable API error was not returned")

    _run_c6_corpus(library_path)


def _run_c6_corpus(library_path: str) -> None:
    """Run every generated C6 request through the actual C5 dispatcher.

    The C5 verifier fixture derives the expected IR from the checked-in SKRQ
    request.  Consequently a Python encoder that drops a field produces a
    scripted-backend mismatch immediately instead of being accepted as a
    transport-only success.
    """
    fixture_root = Path(__file__).resolve().parents[3] / "reference/json-conformance/generated/requests"
    raw_root = Path(__file__).resolve().parents[3] / "core/crates/searchkernel-json/tests/fixtures/requests"
    cases = sorted(fixture_root.glob("*.json"))
    if not cases:
        raise AssertionError(f"C6 request corpus is empty: {fixture_root}")

    for fixture in cases:
        operation, request = request_from_json_text(fixture.read_text(encoding="utf-8"))
        raw_fixture = raw_root / f"{fixture.stem}.skrq"
        if not raw_fixture.exists():
            raise AssertionError(f"{fixture.name}: missing canonical raw request {raw_fixture}")
        open_payload = f"c7-auto|{raw_fixture}".encode("utf-8")
        expected_success = _raw_fixture_succeeds(library_path, open_payload, raw_fixture, operation)
        with open_transport(library_path, open_payload) as transport:
            try:
                result = getattr(Client(transport), operation)(request)
            except ClientError as error:
                if error.portable is None:
                    # C5 deliberately reports malformed FlatBuffers and
                    # malformed unions as a transport status.  The positive
                    # cases below still require a complete typed response;
                    # this branch only covers the negative wire corpus.
                    if expected_success or "status 3" not in str(error):
                        raise AssertionError(f"{fixture.name}: non-portable C5 failure") from error
                elif expected_success:
                    raise AssertionError(f"{fixture.name}: generated request did not match C5 semantics") from error
                continue
        if not expected_success:
            raise AssertionError(f"{fixture.name}: invalid C6 request unexpectedly succeeded")
        _assert_non_empty_response(fixture.name, operation, result)


def _raw_fixture_succeeds(library_path: str, open_payload: bytes, raw_fixture: Path, operation: str) -> bool:
    """Classify the frozen C6 fixture using the same C5 handle and context."""
    with open_transport(library_path, open_payload) as transport:
        try:
            response = transport.call(raw_fixture.read_bytes())
            _read_response(response, operation)
        except (ClientError, RuntimeError):
            return False
    return True


def _assert_non_empty_response(case: str, operation: str, result) -> None:
    if operation in ("insert", "upsert", "patch", "delete") and not result.id:
        raise AssertionError(f"{case}: {operation} response has no ID")
    if operation == "get":
        if result.document is None or not result.document.fields:
            raise AssertionError(f"{case}: get response is empty")
    elif operation == "search":
        if not result.hits or not result.facets or not result.hits[0].fields:
            raise AssertionError(f"{case}: search response is not rich")
        if not result.hits[0].highlights or not result.hits[0].highlights[0].fragments:
            raise AssertionError(f"{case}: search highlights are empty")
    elif operation == "analyze" and not result.tokens:
        raise AssertionError(f"{case}: analyze response is empty")
    elif operation == "describe":
        if result.index is None or len(result.index.fields) < 10:
            raise AssertionError(f"{case}: describe response is incomplete")
    elif operation == "inspect_query" and result.resolved_request is None:
        raise AssertionError(f"{case}: inspect response is empty")
