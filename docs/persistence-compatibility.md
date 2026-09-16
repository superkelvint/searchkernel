# Persistent index compatibility

Persistent index compatibility is governed by the storage compatibility
generation in `manifest/index.json`. The current generation is `1`.

The native ABI, Search Kernel package version, and Vespa revision are recorded
as creation provenance only. They are not reopen gates. A native ABI change
must not require a storage-generation change unless the persisted Vespa
representation also changes.

When a future Vespa or adapter change makes generation 1 unreadable, the
implementation must introduce a new generation and update the explicit reader
support table before writing indexes with it. Existing generations must remain
readable for as long as their support is advertised.

New manifests store the canonical Portable API `IndexDefinition` FlatBuffers
payload in `manifest/index-definition.bfbs`. Manifest v2 is read as a legacy
format only; new indexes never write the handwritten JSON definition format.
