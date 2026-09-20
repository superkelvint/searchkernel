# Authoring the M12 direct Vespa oracle

The C++ oracle is part of the specification, not the implementation.

Do not ask the implementation agent to invent its behavior.

## Required modes

The final `m12_tensor_oracle.cpp` must support:

```text
m12_tensor_oracle run SCENARIO_JSON
m12_tensor_oracle prepare SCENARIO_JSON CONFIG_DIR DATA_DIR
```

`run` creates an isolated fixture, executes all scenario steps directly through pinned Vespa C++, and writes canonical M12 output.

`prepare` creates equivalent config + persistent document state and closes the engine. The Rust low-level and adapter drivers open that same state and execute only the query steps.

## Required direct APIs

Use the same direct `DocumentDB` style as M8/M10. Reuse the pinned checkout's own test compile/link command, but do not call Search Kernel code.

The oracle must directly:

1. configure document type `typea` with:
   - integer summary/attribute field `ordinal` for the non-tensor-field negative case;
   - tensor attribute `embedding` of type `tensor<float>(x[3])`;
   - Euclidean distance metric;
   - no HNSW index;
2. feed x/mid/y vectors exactly as frozen in the scenarios;
3. create the query tensor in the request/ranking property map using the pinned Vespa representation;
4. build a nearest-neighbor query node whose document field and query tensor name come from the scenario;
5. execute real `DocumentDB::match`;
6. map returned GIDs to document IDs;
7. emit ordered hit IDs only.

## Important oracle properties

- no Search Kernel headers/libraries;
- no Rust;
- no hand-computed nearest-neighbor order;
- no use of expected JSON;
- no fixture-specific branch that returns x/mid/y without matcher execution;
- source compiled against exactly `reference/vespa/PINNED_VESPA.json`;
- pinned checkout must be clean.

## Why the source is intentionally not pre-filled in this bundle

The provided repository snapshot does not contain the user's uncommitted native tensor work and does not include a built pinned Vespa checkout in this execution environment. Guessing private pinned C++ tensor constructors/query-node APIs would make the verifier less trustworthy.

Author this file directly against the pinned checkout, then run `freeze-goldens.sh` and commit it before production implementation begins.
