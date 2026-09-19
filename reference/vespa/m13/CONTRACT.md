# M13 contract: rank-feature values

M13 is an additional focused smoke/regression verifier for rank-feature value
transport. The authoritative oracle-backed Phase 5 contract is
`reference/vespa/m10/feature-values/verify-feature-values.sh`, which compares
the native and Rust paths with a frozen direct pinned-Vespa golden. M13 does
not define a second normative DOUBLE/DATA result.

At the Vespa boundary, `FeatureSet::Value` has exactly two representations:

* `DOUBLE`, carrying one `double`;
* `DATA`, carrying an opaque byte payload.

The Search Kernel native ABI exposes that boundary through the opaque,
feature-specific `sk_feature_value` handle. A DATA view is borrowed from the
search result and remains valid until that result is destroyed. Tensor
recognition or decoding is a higher-layer concern and is not part of this
contract.

The legacy scalar getter remains supported. It succeeds for DOUBLE and returns
the stable native type status for DATA; DATA must never be reinterpreted as a
double.

The verifier uses a real rank profile that emits one scalar feature and one
data-backed feature, and checks exact scalar/data preservation through the raw
ABI and the safe Rust wrapper.
