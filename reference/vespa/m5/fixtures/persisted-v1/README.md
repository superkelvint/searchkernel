# M5 persisted-v1 fixture

This directory is a preserved `linux-x86_64` native Vespa data directory. It
is not scratch test state and must only be replaced deliberately when the
native ABI or pinned Vespa revision changes.

The fixture metadata, mutation history, logical contents, and verification
searches are recorded in `METADATA.json`. Verification must copy this directory
to a unique temporary directory and query it through the real M2 native path.
