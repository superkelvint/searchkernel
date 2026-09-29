# M13 contract: dense tensor document CRUD

M13 freezes whole-value dense tensor document lifecycle behavior through the
real Vespa document and update paths. The supported document type is a dense
tensor with one indexed dimension, no mapped dimensions, finite cells, and a
cell count matching its type specification.

The acceptance corpus proves, through both the low-level Rust engine API and
the canonical FlatBuffers dispatcher:

- insert and upsert replacement;
- GET/readback of the canonical tensor type specification and every cell;
- whole-field assignment and generic unset/reassign;
- close/reopen persistence;
- delete and not-found behavior;
- nearest-neighbor ordering immediately after a document tensor update, before
  close/reopen, with the same ordering preserved after reopen.

The positive corpus is independently cross-checked against
`oracle/m13_tensor_oracle.cpp`, which uses the pinned Vespa document,
`TensorFieldValue`, `DocumentUpdate`, assign, and clear APIs directly. The
portable driver creates its first backend by lowering the portable
`IndexDefinition` through `compile_index_definition()`; it does not reuse the
low-level Rust schema helper.

M13 does not add tensor algebra, cell-level updates, sparse/mixed/mapped
document tensors, or a parallel Search Kernel document store. Tensor values
are constructed and validated by the native Vespa adapter and read back as
the existing `TensorValue::Dense` representation.

`scenarios/errors.json` freezes deterministic categories through both paths for
scalar/tensor type mismatches, malformed or unsupported tensor shapes,
cell-count mismatch, non-finite cells, sparse/mixed values, and incompatible
patch values. The error driver compares native detail codes or portable error
detail codes, not merely non-empty diagnostics.
