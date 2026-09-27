# Static Search Kernel Spike

This standalone M1 experiment measures whether the existing Vespa-backed
native runtime can be delivered as a static-ish final executable, one
mostly-static Search Kernel library, or the current relocatable shared closure.
It does not alter production runtime packaging.

Run from the repository root after the normal native SDK prerequisites are
available:

```bash
spikes/static-searchkernel/spike.sh all
```

Individual candidates are available as `candidate-a`, `candidate-b`, and
`candidate-c`. Generated binaries and diagnostics are written below `build/`
and are intentionally ignored. The smoke program uses the public Rust API to
create, populate, query, close, reopen, and query a real embedded Vespa index.
