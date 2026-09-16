# M5 persisted-v1 fixture

This directory is a preserved `linux-x86_64` native Vespa data directory. It
is not scratch test state and must only be replaced deliberately when the
persisted representation or pinned Vespa revision changes. A native ABI change
alone must not replace it.

The fixture metadata, including its historical ABI 4 provenance, mutation
history, logical contents, and verification searches are recorded in
`METADATA.json`. Verification must copy this directory to a unique temporary
directory and query it through the current native path; the fixture bytes and
provenance must not be rewritten to match the current ABI.
