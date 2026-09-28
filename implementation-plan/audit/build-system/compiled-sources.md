# Compiled-source inventory

This table enumerates every compiled-language source under `reference/vespa`
at audited `main` commit `287fe2413e0af376340d59b39146791fd79e9935`. It is
evidence for the 55 active + 14 oracle + 1 uncertain split documented in
`README.md`.

| Path | Language | Role | Classification |
| --- | --- | --- | --- |
| `reference/vespa/api-contract/src/bin/m-api.rs` | Rust | compile-only API fixture; no normal entrypoint found | **DEAD / SUSPECTED DEAD** |
| `reference/vespa/api-contract/src/bin/m1.rs` | Rust | active current-implementation verifier source | **TEMPORARY COMPATIBILITY** |
| `reference/vespa/api-contract/src/bin/m10.rs` | Rust | active current-implementation verifier source | **TEMPORARY COMPATIBILITY** |
| `reference/vespa/api-contract/src/bin/m11.rs` | Rust | active current-implementation verifier source | **TEMPORARY COMPATIBILITY** |
| `reference/vespa/api-contract/src/bin/m12.rs` | Rust | active current-implementation verifier source | **TEMPORARY COMPATIBILITY** |
| `reference/vespa/api-contract/src/bin/m13.rs` | Rust | active current-implementation verifier source | **TEMPORARY COMPATIBILITY** |
| `reference/vespa/api-contract/src/bin/m14.rs` | Rust | active current-implementation verifier source | **TEMPORARY COMPATIBILITY** |
| `reference/vespa/api-contract/src/bin/m2.rs` | Rust | active current-implementation verifier source | **TEMPORARY COMPATIBILITY** |
| `reference/vespa/api-contract/src/bin/m3.rs` | Rust | active current-implementation verifier source | **TEMPORARY COMPATIBILITY** |
| `reference/vespa/api-contract/src/bin/m4.rs` | Rust | active current-implementation verifier source | **TEMPORARY COMPATIBILITY** |
| `reference/vespa/api-contract/src/bin/m6.rs` | Rust | active current-implementation verifier source | **TEMPORARY COMPATIBILITY** |
| `reference/vespa/api-contract/src/bin/m7.rs` | Rust | active current-implementation verifier source | **TEMPORARY COMPATIBILITY** |
| `reference/vespa/api-contract/src/bin/m8.rs` | Rust | active current-implementation verifier source | **TEMPORARY COMPATIBILITY** |
| `reference/vespa/api-contract/src/bin/m9.rs` | Rust | active current-implementation verifier source | **TEMPORARY COMPATIBILITY** |
| `reference/vespa/i/rust-driver/src/main.rs` | Rust | active current-implementation verifier source | **TEMPORARY COMPATIBILITY** |
| `reference/vespa/m0/native-probe/probe.c` | C | active current-implementation verifier source | **TEMPORARY COMPATIBILITY** |
| `reference/vespa/m1/rust-driver/src/main.rs` | Rust | active current-implementation verifier source | **TEMPORARY COMPATIBILITY** |
| `reference/vespa/m10/feature-values/native-probe/probe.c` | C | active current-implementation verifier source | **TEMPORARY COMPATIBILITY** |
| `reference/vespa/m10/oracle/m10_ranking_oracle.cpp` | C++ | oracle/golden-authoring source | **ORACLE AUTHORING** |
| `reference/vespa/m10/rust-driver/src/bin/config_regressions.rs` | Rust | active current-implementation verifier source | **TEMPORARY COMPATIBILITY** |
| `reference/vespa/m10/rust-driver/src/bin/driver.rs` | Rust | active current-implementation verifier source | **TEMPORARY COMPATIBILITY** |
| `reference/vespa/m10/rust-driver/src/bin/error.rs` | Rust | active current-implementation verifier source | **TEMPORARY COMPATIBILITY** |
| `reference/vespa/m10/rust-driver/src/lib.rs` | Rust | active current-implementation verifier source | **TEMPORARY COMPATIBILITY** |
| `reference/vespa/m11/rust-driver/src/main.rs` | Rust | active current-implementation verifier source | **TEMPORARY COMPATIBILITY** |
| `reference/vespa/m12/oracle/m12_tensor_oracle.cpp` | C++ | oracle/golden-authoring source | **ORACLE AUTHORING** |
| `reference/vespa/m12/rust-driver/src/bin/adapter_driver.rs` | Rust | active current-implementation verifier source | **TEMPORARY COMPATIBILITY** |
| `reference/vespa/m12/rust-driver/src/bin/driver.rs` | Rust | active current-implementation verifier source | **TEMPORARY COMPATIBILITY** |
| `reference/vespa/m12/rust-driver/src/bin/error.rs` | Rust | active current-implementation verifier source | **TEMPORARY COMPATIBILITY** |
| `reference/vespa/m12/rust-driver/src/bin/prepare_fixture.rs` | Rust | active current-implementation verifier source | **TEMPORARY COMPATIBILITY** |
| `reference/vespa/m12/rust-driver/src/lib.rs` | Rust | active current-implementation verifier source | **TEMPORARY COMPATIBILITY** |
| `reference/vespa/m13/oracle/m13_tensor_oracle.cpp` | C++ | oracle/golden-authoring source | **ORACLE AUTHORING** |
| `reference/vespa/m13/rust-driver/src/bin/adapter_driver.rs` | Rust | active current-implementation verifier source | **TEMPORARY COMPATIBILITY** |
| `reference/vespa/m13/rust-driver/src/bin/driver.rs` | Rust | active current-implementation verifier source | **TEMPORARY COMPATIBILITY** |
| `reference/vespa/m13/rust-driver/src/bin/error.rs` | Rust | active current-implementation verifier source | **TEMPORARY COMPATIBILITY** |
| `reference/vespa/m13/rust-driver/src/lib.rs` | Rust | active current-implementation verifier source | **TEMPORARY COMPATIBILITY** |
| `reference/vespa/m14/oracle/m14_hnsw_oracle.cpp` | C++ | oracle/golden-authoring source | **ORACLE AUTHORING** |
| `reference/vespa/m14/rust-driver/src/bin/adapter_driver.rs` | Rust | active current-implementation verifier source | **TEMPORARY COMPATIBILITY** |
| `reference/vespa/m14/rust-driver/src/bin/driver.rs` | Rust | active current-implementation verifier source | **TEMPORARY COMPATIBILITY** |
| `reference/vespa/m14/rust-driver/src/bin/error.rs` | Rust | active current-implementation verifier source | **TEMPORARY COMPATIBILITY** |
| `reference/vespa/m14/rust-driver/src/lib.rs` | Rust | active current-implementation verifier source | **TEMPORARY COMPATIBILITY** |
| `reference/vespa/m2/oracle/embedded_crud_spike.cpp` | C++ | oracle/golden-authoring source | **ORACLE AUTHORING** |
| `reference/vespa/m2/rust-driver/src/main.rs` | Rust | active current-implementation verifier source | **TEMPORARY COMPATIBILITY** |
| `reference/vespa/m3/kstem/oracle/KStemOracle.java` | Java | oracle/golden-authoring source | **ORACLE AUTHORING** |
| `reference/vespa/m3/kstem/rust/kstem-rust.rs` | Rust | active current-implementation verifier source | **TEMPORARY COMPATIBILITY** |
| `reference/vespa/m3/oracle/src/searchkernel/m3/LinguisticsOracle.java` | Java | oracle/golden-authoring source | **ORACLE AUTHORING** |
| `reference/vespa/m3/rust-driver/src/main.rs` | Rust | active current-implementation verifier source | **TEMPORARY COMPATIBILITY** |
| `reference/vespa/m4/oracle/m4_text_oracle.cpp` | C++ | oracle/golden-authoring source | **ORACLE AUTHORING** |
| `reference/vespa/m4/rust-driver/src/main.rs` | Rust | active current-implementation verifier source | **TEMPORARY COMPATIBILITY** |
| `reference/vespa/m5/rust-driver/src/main.rs` | Rust | active current-implementation verifier source | **TEMPORARY COMPATIBILITY** |
| `reference/vespa/m6/oracle/src/searchkernel/m6/SchemaOracle.java` | Java | oracle/golden-authoring source | **ORACLE AUTHORING** |
| `reference/vespa/m6/rust-driver/src/main.rs` | Rust | active current-implementation verifier source | **TEMPORARY COMPATIBILITY** |
| `reference/vespa/m7/oracle/src/searchkernel/m7/QueryOracle.java` | Java | oracle/golden-authoring source | **ORACLE AUTHORING** |
| `reference/vespa/m7/rust-driver/src/error.rs` | Rust | active current-implementation verifier source | **TEMPORARY COMPATIBILITY** |
| `reference/vespa/m7/rust-driver/src/main.rs` | Rust | active current-implementation verifier source | **TEMPORARY COMPATIBILITY** |
| `reference/vespa/m8/oracle/m8_local_oracle.cpp` | C++ | oracle/golden-authoring source | **ORACLE AUTHORING** |
| `reference/vespa/m8/rust-driver/src/bin/driver.rs` | Rust | active current-implementation verifier source | **TEMPORARY COMPATIBILITY** |
| `reference/vespa/m8/rust-driver/src/bin/error.rs` | Rust | active current-implementation verifier source | **TEMPORARY COMPATIBILITY** |
| `reference/vespa/m8/rust-driver/src/lib.rs` | Rust | active current-implementation verifier source | **TEMPORARY COMPATIBILITY** |
| `reference/vespa/m9/oracle/m9_attribute_grouping_oracle.cpp` | C++ | oracle/golden-authoring source | **ORACLE AUTHORING** |
| `reference/vespa/m9/rust-driver/src/bin/driver.rs` | Rust | active current-implementation verifier source | **TEMPORARY COMPATIBILITY** |
| `reference/vespa/m9/rust-driver/src/bin/error.rs` | Rust | active current-implementation verifier source | **TEMPORARY COMPATIBILITY** |
| `reference/vespa/m9/rust-driver/src/lib.rs` | Rust | active current-implementation verifier source | **TEMPORARY COMPATIBILITY** |
| `reference/vespa/m9-general-grouping-verifier/native-probe/probe.c` | C | active current-implementation verifier source | **TEMPORARY COMPATIBILITY** |
| `reference/vespa/m9-general-grouping-verifier/oracle/m9_recursive_grouping_oracle.cpp` | C++ | oracle/golden-authoring source | **ORACLE AUTHORING** |
| `reference/vespa/phase9/native-probe/probe.c` | C | active current-implementation verifier source | **TEMPORARY COMPATIBILITY** |
| `reference/vespa/recovery/rust-driver/src/main.rs` | Rust | active current-implementation verifier source | **TEMPORARY COMPATIBILITY** |
| `reference/vespa/searchreply/native-probe/searchreply_metadata_hardening_probe.cpp` | C++ | active current-implementation verifier source | **TEMPORARY COMPATIBILITY** |
| `reference/vespa/searchreply/native-probe/searchreply_probe.cpp` | C++ | active current-implementation verifier source | **TEMPORARY COMPATIBILITY** |
| `reference/vespa/searchreply/oracle/searchreply_oracle.cpp` | C++ | oracle/golden-authoring source | **ORACLE AUTHORING** |
| `reference/vespa/searchreply/rust-driver/src/main.rs` | Rust | active current-implementation verifier source | **TEMPORARY COMPATIBILITY** |
