# Persistence compatibility fixtures

Each version directory is a complete persisted Search Kernel index fixture:
checked-in manifest state, frozen generated engine config, and the preserved M5
Vespa storage payload. Tests copy a complete fixture before mutation; they do
not reconstruct historical state at runtime.

The corpus is append-only. Once a version fixture lands, do not regenerate or
edit it in place. A new manifest/storage generation adds a new directory.

The metadata records manifest-contract provenance separately from the older M5
storage provenance because these compatibility fixtures were assembled once
from preserved historical components rather than captured by a single original
process invocation.
