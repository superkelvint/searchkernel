# Daemon index discovery

`ListIndexesRequest` is a server-scoped portable operation. Its envelope needs
no target. `ListIndexesResult.targets` contains logical target names in lexical
order, with no definitions, paths or generation tokens. Discovery does not open
an index. OpenIndex remains authoritative for existence, its definition and
the generation to use on subsequent target-bound requests.

The daemon enumerates candidate names beneath its configured data root and
uses the canonical IR target validator. Each candidate is inspected under the
existing per-target create/open/delete transition lock. Storage identification
belongs to `Index::inspect_storage`: it reads the same native manifest and
validates the same resolved configuration identity as reopen, without opening
Vespa or acquiring its lifetime lock. Roots, data directories and metadata trees
must be ordinary filesystem entries; symlinks, malformed metadata, incomplete
indexes, internal staging/tombstone/lock artifacts and invalid targets are
excluded. Root enumeration failures return a portable Unavailable error without
filesystem paths.

Listing is not a global transaction. A target may disappear after inspection or
be created after enumeration. OpenIndex therefore still validates the selected
target. Filesystem inspection never holds the registry map lock or a global
transition lock, and does not bind/open all candidates. A daemon restart does
not affect durable discovery.

Public APIs are TypeScript `Daemon.listIndexes()` (browser gRPC-Web by default),
Python `Client.list_indexes()`, Ruby `Client#list_indexes`, PHP
`Client::listIndexes()` and Rust `RemoteClient::list_indexes()`. All use the same
portable operation; none apply target grammar in the client. Discovery is
separate from the existing singular-index lifecycle compatibility contract.

TypeScript `Index.open()` retains the OpenIndex definition as `index.definition`
and binds the returned generation. `SearchBuilder.summary(name)` sets canonical
presentation.summary; omitting ranking, summary or limit leaves server resolution
unchanged.
