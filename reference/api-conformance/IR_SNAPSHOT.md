# Internal IR snapshot format

The v0.6 goldens are an internal observation format for successful V2
`*_to_ir()` conversions. They are not public JSON, transport JSON, or a
backwards-compatible client contract.

Each serializer observes the owned IR value after conversion. Object keys are
fixed by the serializer, maps are emitted in sorted order, and enum values use
explicit `kind` tags where the IR has variants. Empty collections and absent
optional values are retained as `[]` and `null` so that a meaningful change in
IR state changes the snapshot.

Unsupported V2 input remains a negative case with the normal conversion error;
the corpus does not add a synthetic IR representation for rejected input.

When the IR grows, its canonical snapshot serializer must destructure the new
field or exhaustively match the new variant. This makes a missing observation a
compile-time maintenance task rather than a silent conformance gap.
