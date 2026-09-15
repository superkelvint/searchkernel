# M1 contract — Rust engine ownership

## Goal

Own the M0 native engine safely from the frozen public Rust API.

Required public API is defined by `../API_CONTRACT.md` and mechanically checked
by `../api-contract/src/bin/m1.rs`.

## Acceptance

The M1 Rust driver must:

1. load native config with `EngineConfig::from_directory`;
2. call `Engine::open` on a fresh data directory;
3. drop the engine;
4. open the same directory again;
5. drop again;
6. exit successfully.

The executable must dynamically depend on `libsearchkernel_vespa.so`.

## Ownership requirements

- `Engine` owns one native handle.
- Drop closes that handle exactly once.
- native exceptions/errors are translated into `searchkernel::Error`.
- no public raw pointer is exposed.
- no Vespa C++ type appears in the public Rust API.

## Does not count

- a Rust-only fake engine;
- opening only the old standalone C++ test executable;
- shelling out to a Vespa process;
- requiring users of `searchkernel` to call `searchkernel-vespa-bindings`
  directly.
