"""Copy this shape to clients/python/tests/c7_adapter.py when implementing C7.

This is documentation/compile guidance, not production code.
The adapter MUST delegate to the production NativeTransport.
"""


def expected_abi_version() -> int:
    raise NotImplementedError


def open_transport(library_path: str, open_payload: bytes):
    """Return the production NativeTransport instance."""
    raise NotImplementedError


def real_c5_smoke(library_path: str) -> None:
    """Run representative generated-client operations through real C5.

    Minimum: one mutation-like request, one search request, and one
    describe/inspect request against the C3 recording/scripted backend fixture.
    Portable errors must be mapped by generated client code, not transport code.
    """
    raise NotImplementedError
