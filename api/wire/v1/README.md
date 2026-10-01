# Wire V1 schema source

This directory is the single authoritative FlatBuffers schema source for Searchkernel wire major 1.

Compatible FlatBuffers evolution for wire V1 happens here. The top-level `api/searchkernel*.fbs` paths are symlink aliases to these files for path compatibility only; they are not independent schema sources and must not be edited separately.

If an incompatible representation is introduced, preserve this directory and add a sibling `api/wire/v2/` source plus a new codec. Do not overwrite V1 or rename public/semantic APIs.
