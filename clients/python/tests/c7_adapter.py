"""C7 adapter for the production Python client and native transport."""

from __future__ import annotations

import sys
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
)


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
