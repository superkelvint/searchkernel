"""Required shape for clients/python/tests/c7_adapter.py.

This file is documentation/compile guidance, not production code.
The repository adapter MUST delegate to production NativeTransport and
production/generated client code. It MUST NOT implement FFI itself.
"""


def expected_abi_version() -> int:
    """Return the C5 ABI version expected by production NativeTransport."""
    raise NotImplementedError


def open_transport(library_path: str, open_payload: bytes):
    """Return the production NativeTransport instance.

    Required behavior of the returned object:

        call(request: bytes) -> bytes
        close() -> None
        __enter__ / __exit__ context-manager support

    close() must be idempotent and call-after-close must fail locally.
    """
    raise NotImplementedError


def real_c5_smoke(library_path: str) -> None:
    """Exercise generated Python client core through production NativeTransport.

    Minimum coverage:

    - one mutation-like operation;
    - one search operation;
    - one describe or inspect operation;
    - one scripted Portable API error mapped by generated client code.

    The backend must be the real C5 canonical dispatcher wired to C3
    RecordingBackend / ScriptedBackend fixtures. No Vespa dependency is needed.
    """
    raise NotImplementedError
