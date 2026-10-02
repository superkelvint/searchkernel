#!/usr/bin/env python3
"""Regenerate checked-in request envelopes with the current wire major."""

from __future__ import annotations

import sys
from pathlib import Path

import flatbuffers

from searchkernel._wire.v1.generated.SearchKernel.Protocol.RequestEnvelope import RequestEnvelopeT


def regenerate(path: Path) -> None:
    envelope = RequestEnvelopeT.InitFromPackedBuf(path.read_bytes())
    if envelope.protocolVersion == 0:
        raise ValueError(f"{path} has no explicit wire major")
    envelope.protocolVersion = 1

    builder = flatbuffers.Builder(1024)
    offset = envelope.Pack(builder)
    builder.Finish(offset, file_identifier=b"SKRQ")
    path.write_bytes(bytes(builder.Output()))


def main() -> None:
    if len(sys.argv) != 2:
        raise SystemExit("usage: regenerate-request-fixtures.py REQUEST_FIXTURE_DIR")
    root = Path(sys.argv[1])
    for path in sorted(root.glob("*.skrq")):
        regenerate(path)


if __name__ == "__main__":
    main()
