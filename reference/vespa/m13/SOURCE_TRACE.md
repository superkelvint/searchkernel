# M13 source trace

The pinned Vespa source remains authoritative for the underlying behavior:

- `upstream/vespa/document/src/vespa/document/fieldvalue/tensorfieldvalue.h`
  defines `document::TensorFieldValue`, which owns document tensor values and
  exposes the evaluated tensor through `getAsTensorPtr()`;
- `upstream/vespa/eval/src/vespa/eval/eval/value.h` and
  `upstream/vespa/eval/src/vespa/eval/eval/typed_cells.h` expose
  `vespalib::eval::Value`, its `ValueType`, and its typed cell buffer;
- `upstream/vespa/document/src/vespa/document/update/assignvalueupdate.h` and
  `clearvalueupdate.h` define whole-field assignment and clear semantics;
- `upstream/vespa/document/src/tests/documentupdatetestcase.cpp` covers the
  Vespa tensor assign/clear update behavior used by the direct M13 oracle;
- `reference/vespa/m13/oracle/m13_tensor_oracle.cpp` exercises those Vespa
  document and update classes independently of Search Kernel;
- Search Kernel's C++ adapter decodes/encodes those Vespa values and copies
  dense cells into Search Kernel-owned primitive storage for the ABI lifetime;
- document PUT, update assignment, clear, retrieval, persistence, and vector
  matching remain Vespa operations.

Search Kernel-owned additions are limited to the ABI accessors
`sk_document_field_tensor_dense` and
`sk_document_update_assign_tensor_dense`. Rust reconstructs the existing
`TensorValue::Dense`; the portable adapter maps it to the existing V2 tensor
IR and FlatBuffers response representation.
