# `searchkerneld`

`searchkerneld` is the Search Kernel daemon and transport host.

Run it against an existing index with:

```text
searchkerneld --index /path/to/index --listen unix:///tmp/searchkerneld.sock
```

For a new index, pass its canonical FlatBuffers definition with
`--definition`; that option is only used during creation. On Unix, omitting
`--listen` uses `/tmp/searchkerneld.sock`.

Its responsibilities are HTTP routing, standard FlatBuffers JSON/FlatBuffers
content negotiation, connection and lifecycle management, capability
reporting, and process-local diagnostics. JSON is converted to canonical
FlatBuffer bytes before dispatch. Portable requests must enter the same
canonical dispatcher used by native clients; the daemon must not implement a
second semantic path.

The transport host is tested with recording and scripted backends. Production
backend wiring belongs at the daemon composition boundary and uses
`core/crates/searchkernel-ir-adapter`.
