  ──────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────
   Rank      2
   Area      Native C++ adapter
   Evidence  core/native/vespa/searchkernel_vespa.cpp:3526 is ~5,300 lines with no C++ unit-test target or native line coverage.
   Risk      Highest consequence: ABI corruption, leaks, exception escape, invalid result pointers, malformed UTF-8, and concurrency bugs.
             M11 has strong stress/sanitizer scenarios, but they are not fine-grained unit tests.
  ──────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────
   Rank      3
   Area      Vespa Rust wrapper/result decoding
   Evidence  core/crates/searchkernel-vespa-engine/src/lib.rs:648 is only ~44% line-covered. Query/schema are ~85%; the uncovered area is
             mainly engine operations and defensive native-result handling.
   Risk      Native null/invalid pointers, unknown value kinds, invalid UTF-8, summary/match-feature/grouping decoding, and partial-failure
             cleanup are mostly untested.
  ──────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────
   Rank      4
   Area      Portable API ↔ IR conversion
   Evidence  api_to_ir.rs is ~37% covered; ir_to_api.rs ~48%. The 34 semantic tests are useful, but the large conformance corpus runs
             outside normal cargo test.
   Risk      Silent semantic drift in presence/default handling, malformed unions, nested values, duplicate fields, and error metadata.
  ──────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────
   Rank      5
   Area      Generated Rust client
   Evidence  core/crates/searchkernel-client/tests/behavior.rs:77 has one test covering all sync operations through a scripted success
             response. AsyncClient is untested.
   Risk      Transport errors, malformed responses, protocol errors, async behavior, and less-common nested/request variants.
  ──────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────
   Rank      6
   Area      Testkit conformance runner
   Evidence  core/crates/searchkernel-testkit/src/runner.rs:341 has effectively 0% execution coverage in workspace tests.
   Risk      Test infrastructure could miss or misreport conformance failures; this does not directly affect production behavior.

  The highest-priority test work would be:

  1. Direct BackendSession tests using barriers: concurrent close, close failure, callback panic, operation error, and repeated drop/close.
  2. Native adapter hardening tests for every public accessor’s null/index/type/UTF-8/error path, under ASan/UBSan/TSan.
  3. Fault-injection tests for Rust native-result decoding.
  4. Run the API conformance corpus under coverage, then add property/fuzz tests for FlatBuffers boundary cases.