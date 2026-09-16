# C7 source trace

C7 is a Search Kernel-owned transport milestone; there is no upstream Vespa
oracle.

Normative inputs:

- C5 public native ABI (`searchkernel-ffi`);
- C6 generated client core and Transport abstraction;
- C3 RecordingBackend / ScriptedBackend test infrastructure;
- canonical FlatBuffers request/response envelopes from the Portable API.

Verifier-only instrumentation:

- `shim/c7_ffi_shim.c` implements only observable transport mechanics;
- it is not an acceptance replacement for the real C5 composition path;
- it must never parse FlatBuffers or implement Search Kernel operations.
