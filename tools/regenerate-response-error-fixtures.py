#!/usr/bin/env python3
"""Regenerate checked-in response envelopes with the current wire major."""

from __future__ import annotations

import sys
from pathlib import Path

import flatbuffers

from searchkernel._wire.v1.generated.SearchKernel.Protocol.ResponseEnvelope import ResponseEnvelopeT


def regenerate(path: Path) -> None:
    envelope = ResponseEnvelopeT.InitFromPackedBuf(path.read_bytes())
    if envelope.payload is None:
        raise ValueError(f"{path} has no response payload")
    envelope.protocolVersion = 1

    builder = flatbuffers.Builder(1024)
    offset = envelope.Pack(builder)
    builder.Finish(offset, file_identifier=b"SKRS")
    path.write_bytes(bytes(builder.Output()))


def main() -> None:
    if len(sys.argv) != 2:
        raise SystemExit("usage: regenerate-response-error-fixtures.py RESPONSE_FIXTURE_DIR")
    root = Path(sys.argv[1])
    for path in sorted(root.glob("*.skrs")):
        regenerate(path)


if __name__ == "__main__":
    main()
