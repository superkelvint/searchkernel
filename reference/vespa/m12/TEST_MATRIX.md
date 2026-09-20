# M12 test matrix

## Positive execution

| Scenario | Low-level | FB -> IR -> adapter | Direct C++ oracle | What it proves |
|---|---:|---:|---:|---|
| `exact-near-far` | yes | yes | yes | basic dense input + exact NN ordering |
| `query-vector-changes-order` | yes | yes | yes | tensor cells actually affect matching |
| `multiple-query-tensors` | yes | yes | yes | query references tensor by name |
| `scalar-and-tensor-inputs` | yes | yes | yes | M10 scalar and M12 tensor inputs coexist |

The positive fixture is deliberately attribute-only, no HNSW index. Ordered IDs are semantic. Exact score bits and total-hit count are not frozen in M12 v1.

## Negative validation

| Case | Expected category | Expected owner |
|---|---|---|
| empty tensor input name | `invalid_query_tensor` | native M |
| duplicate tensor input name | `duplicate_query_tensor` | native M |
| scalar/tensor same name | `query_input_collision` | native M / adapter forwarding |
| rank-property/tensor same name | `query_input_collision` | native M / adapter forwarding |
| malformed type spec | `invalid_query_tensor` | native M |
| two indexed dimensions | `invalid_query_tensor` | native M |
| mapped dimension | `invalid_query_tensor` | native M |
| too few dense cells | `invalid_query_tensor` | native M |
| too many dense cells | `invalid_query_tensor` | native M |
| NaN cell through low-level API | `invalid_query_tensor` | native M |
| Infinity cell through low-level API | `invalid_query_tensor` | native M |
| portable NaN/non-finite tensor encoding | `non_finite_tensor_cell` | C2/IR |
| NN references missing input | `query_tensor_missing` | C2/IR |
| expression input | `expression_query_input_not_supported` | adapter |
| sparse literal tensor | `sparse_query_tensor_not_supported` | adapter |
| mixed literal tensor | `mixed_query_tensor_not_supported` | adapter |
| unknown document field | `query_tensor_field_invalid` | native M |
| non-tensor document field | `query_tensor_field_invalid` | native M |
| query/document tensor type mismatch | `query_tensor_type_mismatch` | native M |

## Structural/anti-shortcut checks

Acceptance also checks:

- M11 is run first;
- low-level driver dynamically links `libsearchkernel_vespa`;
- low-level driver references the native query-tensor symbol(s);
- low-level driver references the native nearest-neighbor symbol(s);
- production M crates do not depend on `searchkernel-ir` or testkit;
- production code does not reference `reference/vespa/m12`, scenario IDs, or expected outputs;
- no Rust distance/scanning helper is introduced in the adapter or low-level wrapper;
- adapter source contains both a scalar query-feature path and dense query-tensor path;
- adapter does not contain a tensor-type grammar/parser.
