# Query diagnostics implementation and verification

W5's product and backend specifications remain authoritative. This document
records the implementation boundary and the evidence still needed for closure.
A successful unit test or a displayed trace does not establish W5 completion.

## Public API and execution

`QueryDiagnosticsRequest` and `QueryDiagnosticsResult` are appended to the
canonical request/response unions. Requests carry a canonical `SearchRequest`,
an intent, evidence sections, an optional original run token, selected hits,
an expected document ID, an experimental request, and typed diagnostic limits.
The envelope must carry the logical target and the generation returned by
`OpenIndex`. Both the canonical decoder and dispatcher reject unbound diagnostics.

Intents are ExplainHit, ZeroResults, MissingDocument, RankingComparison,
Overmatching, QueryInterpretation, VectorRetrieval, Performance, DeepEngine, and
Experiment. An intent selects backend presets, not browser-owned Vespa knobs.
An unsupported section remains explicitly unsupported; other evidence survives.

Ordinary search does not trace or profile. The optional `diagnostic_source`
control retains bounded original execution identity and returned IDs/scores.
Search Lab opts in; ordinary SDK searches preserve their existing response shape.
The per-dispatcher source cache holds at most 16 runs, with 100 returned hits and
bounded request/snapshot size per entry. Eviction makes original evidence
unavailable. Tokens from another dispatcher/incarnation are rejected.
Diagnostics never reconnect to a new generation or mutate index configuration.

The canonical IR owns findings, comparison sets, request hashes, tri-state
observations, controlled elimination variants, and exact-NN variants. Every
finding references evidence IDs. A non-cryptographic request hash identifies
request content; it is not an authorization credential.

Executions distinguish Original, DiagnosticRerun, Experiment, and Counterfactual.
The current adapter does not implement forced-document evaluation: Counterfactual
is explicitly unsupported. No rerun features are attached to original execution
hits. Overlap and movements compare returned sets, not corpus recall. Incomplete
runs cannot establish absent candidate or returned-hit evidence.

## Normalized and engine-specific evidence

Stable structures include execution identity, findings, evidence items, hit
signals, returned-set ordinals/ties, expected-document stages, result divergence,
and section availability. Provenance is Profile, Engine, or Calculated.
Configured phase facts do not imply per-hit phase participation or causality.

`searchkernel-ir-adapter` maps typed evidence requests to engine diagnostic
controls. ABI 17 projects matching/first-/second-phase profiler, optimized
blueprint, and iterator structures in native C++ before raw trace encoding.
Rust maps Search Kernel-owned node records and independent statuses into portable
evidence; it does not interpret engine trace tags. Each section carries engine,
pinned revision, and ABI provenance. See `core/native/vespa/README.md` for the
pinned paths, bounds, and topology limitations.

Raw trace remains separate `RawEngineDiagnostics` with execution ID, media type,
and status. Raw truncation cannot change structured section status. Profiler
report-root totals may be summed across captured threads; descendant totals are
never included in that sum. This measures profiled thread time, not wall-clock
latency. Missing self time remains absent.

Intentional configured match features supply scalar and supported dense tensor
signals. Tensor decoding uses Vespa's binary tensor codec at the opaque native
ABI boundary. Unsupported tensor representations are omitted with explicit
unavailable-feature metadata, never replaced with a fabricated feature value.
Returned-set statistics include the comparison set and count. No magnitude is
presented as causal importance.

Defaults: 5-second total deadline, 8 variants, 20 feature hits, 256 tensor cells,
65536 raw bytes, trace ceiling 9, profiler depth 4, exact-NN bound 100.
Maximums: 30 seconds, 16 variants, 100 feature hits, 4096 tensor cells, 262144 raw
bytes, trace ceiling 9, profiler depth 16, exact-NN bound 1000. Canonical baseline
and experimental requests are each bounded to 262144 encoded bytes. Trace
serialization is bounded before allocation growth. Oversized native DATA feature
columns are omitted and marked truncated. Diagnostic deadlines/cancellation use
normal operation semantics; incomplete evidence is not reported as complete.

## Search Lab

Diagnostics closed leaves the ordinary search/results workflow intact. Opening
Why this result or What's wrong with this search reveals an adjacent evidence
panel with Explain, Investigate, and Inspect. Selecting/pinning original hits
keeps the result list visible. Adjacent comparison is one action away. The Explain
lens labels its values as diagnostic-rerun evidence. The no-signals state gives
structural evidence and a link to ranking configuration without inventing a
rank-expression explanation.

Investigations include zero results, missing document, ranking, overmatching,
query interpretation, vectors, and performance. Expected documents can be found
through real browse/text search. Constraint variants are backend-created and
show their canonical request and observed result sample. Experiments use an
independent canonical request and never persist ranking/schema changes.

Inspect exposes normalized request/evidence and expandable engine trees.
Profiler collection is progressive. Trees use semantic details/summary controls;
comparison has textual numerical favour/ties; tabs support arrow/Home/End keys;
loading uses a polite live region. Original results remain visible during reruns.
Abort controllers and sequence/scope guards prevent old work from updating a new
query or target generation. Stale generation errors require explicit refresh.

Default shared URLs omit query-input values. Browser history retains private
inputs for reload/back/forward; inclusion in copied URLs requires an explicit
checkbox. Diagnostic bundles require explicit inclusion of query/input values,
document IDs/features, and raw engine evidence. Copy-as-curl targets the public
BFBS-backed `/v1/execute` API and never emits credentials/auth headers.

## Clients and verification

The TypeScript, Python, Ruby, PHP, and Rust remote clients expose the canonical
operation using their existing transport and generation binding. Rust returns an
owned response whose generated view retains the complete canonical result.
Generation uses `./dev test protocol-generation` in exact-SHA CI, including BFBS,
protocol bindings, semantic metadata, native bindings, and native consumer metadata.
Generated files are not edited by hand.

Coverage lives in IR/dispatcher tests, SDK diagnostic transport tests, daemon
multi-index gRPC/HTTP tests, and `searchkernel-web/tests/real-daemon.test.ts`.
The real browser scenario creates and populates a native index, restarts the
daemon, and exercises explanation, named features, comparison, elimination,
expected-document selection, experiments, profiling/vector comparison, truncation,
and deletion/recreation with a stale handle.

Canonical CI lanes: `./dev test ir`, `./dev test rust-api`,
`./dev test typescript`, `./dev test python`, `./dev test ruby`, `./dev test php`,
`./dev test searchkerneld-multi-index`, `./dev test frontend`,
`./dev test frontend-real-engine`, `./dev verify frontend.f8`,
`./dev verify contracts`, `./dev verify api-conformance`,
`./dev verify client-semantic-parity`, and `./dev ci`.
PR verification comments bind every command to a full commit SHA. Consult PR
results for executed commands and exact-head outcomes; this list is not a claim
that they all passed.

## Current authoritative capability gaps

These must remain explicit rather than receive frontend approximations:

- The native schema adapter rejects named rank functions, full rank/summary
  feature declarations, and global phase in `core/crates/searchkernel-ir-adapter/src/schema.rs`.
  Match features and supported first/second phases are available.
- Candidate IDs, per-hit phase admission/movement, and filter-stage observations
  are not exported by the native result boundary. Missing-document evidence
  observes fresh GET and returned status; intermediate stages are unavailable.
- Authoritative matched terms/offsets and forced-document evaluation are not
  acquired. No React string matching or counterfactual score inference is used.
- Exact comparison supports an isolated canonical typed nearest-neighbor root
  within the configured bound. Arbitrary opaque YQL/hybrid trees are not rewritten.
- The native boundary projects optimized blueprint and iterator topology.
  It does not claim per-hit causality, candidate admission, or a full raw engine
  response. The bounded final matcher request remains separate raw evidence.
- Diagnostic tensor decoding currently supports single-indexed dense tensors.
  Other tensor forms remain unavailable rather than lose shape/type information.

## Human usability acceptance remains required

Recruit application/search developers who do not specialize in Vespa. Give each
participant the following tasks from Search Lab, without explaining engine knobs:

1. Explain what is known about a no-signals hit at position four.
2. Find a constraint whose removal is sufficient to produce non-zero results.
3. Identify the first observed divergence for a selected expected document.
4. Compare adjacent hits and name signals numerically favouring each, without
   describing raw feature magnitude as importance.
5. Determine whether approximate/exact comparison changes the returned top ten.
6. Identify the larger measured ranking phase and open the profiler evidence.

Record the fixture/version/generation, participant background, actions,
assistance required, findings cited, rerun/original distinction, and raw-evidence
navigation. Collect baseline observations before choosing any time threshold.
No human session has been conducted by this automated implementation; W5's
usability criterion must not be marked complete without that evidence.
