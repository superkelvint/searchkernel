"""Stable internal codec entrypoint; wire-major selection stays private here."""

from .v1.codec import (
    ERROR_TYPE,
    PROTOCOL_VERSION,
    REQUEST_IDENTIFIER,
    RESPONSE_IDENTIFIER,
    expected_response_type,
    pack_object,
    pack_request,
    unpack_response,
    wire_enum_matches,
)

__all__ = [
    "ERROR_TYPE",
    "PROTOCOL_VERSION",
    "REQUEST_IDENTIFIER",
    "RESPONSE_IDENTIFIER",
    "expected_response_type",
    "pack_object",
    "pack_request",
    "unpack_response",
    "wire_enum_matches",
]
