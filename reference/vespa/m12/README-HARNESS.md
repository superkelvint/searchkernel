# M12 verifier harness

M12 is intentionally split into three independently comparable paths:

```text
A. direct pinned Vespa C++ oracle
B. low-level searchkernel-vespa-engine path
C. canonical FlatBuffers -> IR -> searchkernel-ir-adapter path
```

All three consume the same scenario corpus and the same native fixture state.

## Golden authoring

Before implementation:

1. author and review the direct C++ oracle against the pinned Vespa checkout;
2. run `oracle/build-cpp-oracle.sh`;
3. run `freeze-goldens.sh`;
4. review `expected/*.json`;
5. commit the verifier/goldens;
6. only then start implementation.

`freeze-goldens.sh` is the only script allowed to create/replace positive expected files.

## Fixture strategy

The direct oracle owns fixture creation. It must create an attribute-only tensor field:

```text
tensor<float>(x[3])
distance metric: euclidean
HNSW: disabled
```

and persist exactly these documents:

```text
id:ns:typea::x   [1.0, 0.0, 0.0]
id:ns:typea::mid [0.8, 0.2, 0.0]
id:ns:typea::y   [0.0, 1.0, 0.0]
```

`oracle/prepare-fixture.sh SCENARIO CONFIG_DIR DATA_DIR` must create config and persistent data consumable by both Rust drivers. Rust verifier code does not need to know how tensor documents are fed; M12 is a query-input/query-execution milestone.

## Driver output

Every positive driver emits only:

```json
{
  "id": "scenario-id",
  "steps": [
    {"id":"step-id","hits":["doc-id-1","doc-id-2"]}
  ]
}
```

Do not freeze total-hit count or score bits in M12 v1.
