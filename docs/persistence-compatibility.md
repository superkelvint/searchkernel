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

## Creation publication durability

A successful `Index::create` means the complete staged index directory was
atomically renamed to its final root and the containing directory was
successfully fsynced. This guarantee concerns publication of the index
directory entry; it does not strengthen Vespa's separately documented mutation
durability semantics.

If the parent-directory fsync fails after the rename, creation returns
`Unavailable` with error detail `index_published_durability_uncertain` and
the published root path. The root is not rolled back or deleted, because
publication has already occurred. Callers should reconcile it with
`Index::open`. Repeating `Index::create` against that root is non-destructive
and returns `AlreadyExists`. A failed recovery open also leaves the published
root available for a later retry.
