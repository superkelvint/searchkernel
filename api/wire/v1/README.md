# Wire V1 schema source

This directory is the authoritative FlatBuffers schema source for Searchkernel wire major 1.

Compatible FlatBuffers evolution for wire V1 happens here. The files under `api/` with the same names are compatibility mirrors for repository tooling and must remain byte-for-byte identical; protocol generation checks enforce that invariant.

If an incompatible representation is introduced, preserve this directory and add a sibling `api/wire/v2/` source plus a new codec. Do not overwrite V1 or rename public/semantic APIs.
