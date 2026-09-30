# M12 — Dense query tensors and nearest-neighbor execution

## Goal

M12 freezes the first dense query-tensor / nearest-neighbor execution slice.

The accepted low-level path is:

```text
SearchRequest::query_tensor(name, TensorValue::dense(...))
        +
Query::nearest_neighbor(document_field, query_tensor_name)
        ↓
searchkernel-vespa-engine
        ↓
Search Kernel native ABI
        ↓
Vespa query/ranking properties + query tree
        ↓
real Proton Matcher nearest-neighbor execution
```

The accepted integration path is:

```text
standard FlatBuffers JSON
        ↓
canonical RequestEnvelope bytes
        ↓
searchkernel-ir (*_to_ir)
        ↓
searchkernel-ir-adapter
        ↓
SearchRequest::query_tensor + Query::nearest_neighbor
        ↓
real Proton Matcher
```

M10 and M11 contracts remain unchanged and cumulative.

## Scope

M12 v1 supports **literal dense query tensors only**.

Supported request tensor:

```text
tensor<float>(x[N])
```

with all of the following native constraints:

- non-empty query-input name;
- non-empty type specification;
- exactly one indexed dimension;
- no mapped dimensions;
- positive indexed dimension size;
- dense value count exactly matches the indexed dimension size;
- every cell is finite;
- query tensor name is unique in the effective query/rank-property namespace.

M12 v1 does not add sparse or mixed query tensors.

## Frozen low-level Rust API

The compile-only API contract is:

```text
core/crates/searchkernel-vespa-engine/examples/api-contract-m12.rs
```

M12 requires these low-level operations:

```rust
TensorValue::dense(type_spec, values)
SearchRequest::query_tensor(name, tensor)
Query::nearest_neighbor(document_field, query_tensor_name)
```

No milestone-specific request/result family may be introduced.

## Query input namespaces

M10 scalar query features continue to lower to Vespa scalar query/rank properties.

M12 dense literal tensors lower to the native tensor query-input path.

```text
scalar literal input  -> SearchRequest::query_feature
Dense TensorValue     -> SearchRequest::query_tensor
```

A tensor must never be flattened, stringified, or converted to one or more scalar query features.

Scalar query features, dense query tensors, and raw rank properties share the effective Vespa request property namespace. The same effective name must not be supplied through more than one producer.

The verifier freezes collision rejection between:

- scalar query feature vs tensor query input;
- raw rank property vs tensor query input;
- duplicate tensor query inputs.

## Nearest-neighbor query

M12 lowers the existing nearest-neighbor semantic operator to:

```rust
Query::nearest_neighbor(document_field, query_tensor_name)
```

The query tensor is referenced by **name**. The implementation must not select the first tensor input or inline/copy the tensor into the query tree.

The verifier includes two simultaneously populated tensor inputs and references only one of them.

## Deterministic fixture

M12 uses an attribute-only dense tensor field with an explicit Euclidean distance metric and **no HNSW index**. This keeps the acceptance fixture on Vespa's exact nearest-neighbor path and avoids approximate/HNSW nondeterminism.

Fixture vectors:

```text
id:ns:typea::x     [1.0, 0.0, 0.0]
id:ns:typea::mid   [0.8, 0.2, 0.0]
id:ns:typea::y     [0.0, 1.0, 0.0]
```

The verifier freezes ordered hit IDs only. It deliberately does not freeze total-hit count or exact relevance-score bits for this first NN slice.

## Required positive behavior

M12 must prove all of the following through real Vespa execution:

1. Query `[1,0,0]` orders `x, mid, y`.
2. Query `[0,1,0]` orders `y, mid, x`.
3. Changing only the query tensor changes result ordering.
4. With two dense inputs present, `query_tensor = q_y` uses `q_y`, not the first tensor.
5. A scalar query feature and a differently named tensor input can coexist.
6. The portable FlatBuffers/IR/adapter path produces the same ordered hits as the low-level M12 path.

## Validation ownership

M12 freezes which layer owns rejection so validation does not get duplicated opportunistically.

### Portable IR owns

- malformed/missing FlatBuffers union payloads;
- duplicate portable map keys;
- non-finite portable tensor cells;
- nearest-neighbor references to an undefined query input;
- empty nearest-neighbor field/input names where C2 already validates them.

### IR adapter owns

- expression query inputs remain unsupported;
- sparse literal tensors remain unsupported;
- mixed literal tensors remain unsupported;
- literal value kinds that cannot be lowered to current native query inputs remain unsupported;
- routing scalar literals to `query_feature` and dense tensors to `query_tensor`.

The adapter must not parse Vespa tensor type syntax or duplicate the native dense-shape validator.

### Native M layer owns

- tensor type syntax;
- exactly one indexed dimension;
- no mapped dimensions;
- dense cell-count matching;
- native query-property namespace collisions;
- existence/type compatibility of the document tensor field;
- compatibility between the query tensor and target tensor field.

## Frozen negative categories

The verifier freezes coarse error categories, not human-readable strings.

```text
invalid_query_tensor
duplicate_query_tensor
query_input_collision
query_tensor_missing
query_tensor_type_mismatch
query_tensor_field_invalid
expression_query_input_not_supported
sparse_query_tensor_not_supported
mixed_query_tensor_not_supported
non_finite_tensor_cell
```

See `scenarios/errors.json` and `TEST_MATRIX.md` for layer ownership.

## Explicit non-goals

Not required by M12 v1:

```text
sparse query tensors
mixed query tensors
query-input expressions
embedding functions
HNSW/approximate tuning
targetHits/totalTargetHits annotations
distanceThreshold
nearest-neighbor labels
multiple indexed dimensions
mapped document tensor dimensions
multi-vector NN
portable tensor document mutation
new public Python/Rust facade sugar
```

These may be separate later milestones.

## Required independent oracle

Positive execution is accepted only against a direct C++ oracle built from the pinned Vespa checkout.

For every positive scenario:

```text
frozen expected == direct Vespa C++ oracle
frozen expected == low-level Rust M12 driver
frozen expected == FlatBuffers/IR/adapter driver
oracle          == low-level driver
oracle          == adapter driver
```

The C++ oracle must not link or call Search Kernel production code.

## Forbidden shortcuts

The following fail M12 even if expected hit IDs are produced:

```text
Rust-side vector distance calculation
Rust-side nearest-neighbor scan
adapter-side vector scoring
hard-coded fixture IDs or hit order in production code
calling the oracle from production code
flattening a dense tensor into scalar query features
serializing a tensor as an opaque string rank property
ignoring the nearest-neighbor query tensor name
rewriting nearest-neighbor as ordinary filters
using a fake/in-memory search backend for acceptance
```

The verifier checks native dynamic linkage and required tensor/nearest-neighbor native symbols.
