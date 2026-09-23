# Vespa verifier entry points

Active executable verifier entry points use semantic names that describe what
they prove. Historical milestone packages under `reference/vespa/mN/` remain
frozen specification/oracle provenance and are not renamed merely to improve
current execution ergonomics.

During this migration, semantic entry points delegate to the existing frozen
scripts. Some historical scripts are cumulative, so a semantic target may still
execute predecessor verification until the underlying verifier is safely split.
Where an independently meaningful verifier already exists, such as recursive
grouping or M11 sanitizer coverage, it has its own semantic target.

Use:

```text
./dev verify --list
./dev verify vespa.grouping.yql
./dev verify vespa.hardening.sanitizers
./dev verify vespa.persistence.reopen
./dev verify milestone.m14
```

`milestone.mN` is an aggregate historical closure. Legacy `mN` aliases remain
temporarily supported by `reference/vespa/verify-through.sh`.

Canonical semantic targets:

- `vespa.native-sdk` -> `reference/vespa/verifiers/native-sdk/verify.sh` -> `reference/vespa/m0/verify-m0.sh`
- `vespa.rust-ownership` -> `reference/vespa/verifiers/rust-ownership/verify.sh` -> `reference/vespa/m1/verify-m1.sh`
- `vespa.document-mutation` -> `reference/vespa/verifiers/document-mutation/verify.sh` -> `reference/vespa/m2/verify-m2.sh`
- `vespa.linguistics` -> `reference/vespa/verifiers/linguistics/verify.sh` -> `reference/vespa/m3/verify-m3.sh`
- `vespa.text-indexing` -> `reference/vespa/verifiers/text-indexing/verify.sh` -> `reference/vespa/m4/verify-m4.sh`
- `vespa.persistence.reopen` -> `reference/vespa/verifiers/persistence/reopen/verify.sh` -> `reference/vespa/m5/verify-m5.sh`
- `vespa.schema-config` -> `reference/vespa/verifiers/schema-config/verify.sh` -> `reference/vespa/m6/verify-m6.sh`
- `vespa.query-compilation` -> `reference/vespa/verifiers/query-compilation/verify.sh` -> `reference/vespa/m7/verify-m7.sh`
- `vespa.results` -> `reference/vespa/verifiers/results/verify.sh` -> `reference/vespa/m8/verify-m8.sh`
- `vespa.grouping` -> `reference/vespa/verifiers/grouping/attributes-facets/verify.sh` -> `reference/vespa/m9/verify-m9.sh`
- `vespa.grouping.recursive` -> `reference/vespa/verifiers/grouping/recursive/verify.sh` -> `reference/vespa/m9-general-grouping-verifier/verify-recursive-grouping.sh`
- `vespa.grouping.yql` -> `reference/vespa/verifiers/grouping/yql/verify.sh` -> `reference/vespa/m13/grouping-yql/verify-grouping-yql.sh`
- `vespa.ranking` -> `reference/vespa/verifiers/ranking/verify.sh` -> `reference/vespa/m10/verify-m10.sh`
- `vespa.ranking.feature-values` -> `reference/vespa/verifiers/ranking/feature-values/verify.sh` -> `reference/vespa/m10/feature-values/verify-feature-values.sh`
- `vespa.search-reply` -> `reference/vespa/verifiers/search-reply/verify.sh` -> `reference/vespa/searchreply/verify-searchreply.sh`
- `vespa.hardening` -> `reference/vespa/verifiers/hardening/full/verify.sh` -> `reference/vespa/m11/verify-m11.sh`
- `vespa.hardening.core` -> `reference/vespa/verifiers/hardening/core/verify.sh` -> `reference/vespa/m11/verify-m11-core.sh`
- `vespa.hardening.stress` -> `reference/vespa/verifiers/hardening/stress/verify.sh` -> `reference/vespa/m11/verify-m11-stress.sh`
- `vespa.hardening.sanitizers` -> `reference/vespa/verifiers/hardening/sanitizers/verify.sh` -> `reference/vespa/m11/verify-m11-sanitizers.sh`
- `vespa.nearest-neighbor.query` -> `reference/vespa/verifiers/nearest-neighbor/query/verify.sh` -> `reference/vespa/m12/verify-m12.sh`
- `vespa.tensor.document-crud` -> `reference/vespa/verifiers/tensor/document-crud/verify.sh` -> `reference/vespa/m13/verify-m13.sh`
- `vespa.nearest-neighbor.hnsw` -> `reference/vespa/verifiers/nearest-neighbor/hnsw/verify.sh` -> `reference/vespa/m14/verify-m14.sh`
- `vespa.recovery` -> `reference/vespa/verifiers/recovery/verify.sh` -> `reference/vespa/recovery/verify-recovery.sh`
- `product.rust-real-engine` -> `reference/vespa/verifiers/product/rust-real-engine/verify.sh` -> `reference/vespa/phase10/verify-phase10.sh`
- `product.integration` -> `reference/vespa/verifiers/product/integration/verify.sh` -> `reference/vespa/i/verify-i.sh`
