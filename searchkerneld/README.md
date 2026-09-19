# `searchkerneld`

`searchkerneld` is the Search Kernel daemon and transport host.

Its responsibilities are HTTP routing, standard FlatBuffers JSON/FlatBuffers
content negotiation, connection and lifecycle management, capability
reporting, and process-local diagnostics. JSON is converted to canonical
FlatBuffer bytes before dispatch. Portable requests must enter the same
canonical dispatcher used by native clients; the daemon must not implement a
second semantic path.

The transport host is tested with recording and scripted backends. Production
backend wiring belongs at the daemon composition boundary and uses
`core/crates/searchkernel-ir-adapter`.
