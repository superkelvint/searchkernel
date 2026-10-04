# Forced-document ranking at the pinned Vespa revision

This is the feasibility disposition for [#1052](https://github.com/superkelvint/searchkernel/issues/1052).
The authority is Vespa commit `2300ef0412bdb76f227ae3b497c8ca5e73c5da4e`,
recorded in `core/native/vespa/manifest.json`, and section 14 of the
[W5 backend contract](../implementation-plan/W5%20%E2%80%94%20Query%20Diagnostics%20Backend%20Contract.md).

## Disposition

Keep forced-document ranking **Unsupported in the current embedded integration**.
There is no general native operation that scores an arbitrary document *as though
it matched* while preserving the query and avoiding invented match data. An
executor accepting a document ID is insufficient evidence for that operation.
Do not add a match-all branch, document-ID OR clause, feature override, or
synthetic term positions to obtain a score.

This conclusion is narrower than saying Vespa cannot evaluate any feature for a
nonmatching document. Some feature graphs can be evaluated independently of
retrieval. The current adapter exposes neither a separate evaluation context nor
a validated feature-dependency capability for those graphs. No partial support
is claimed or enabled by this investigation. A numeric result from the ordinary
search/docsum path alone cannot establish counterfactual support.

The W5 implementation is separately under review in
[PR #1043](https://github.com/superkelvint/searchkernel/pull/1043). This disposition
does not require that PR to stop returning other diagnostic sections. Unsupported
counterfactual evidence is section-local; it is neither a failed ordinary search
nor evidence that the expected document did not match.

## What the native evaluator needs

[RankProgram::setup](https://github.com/vespa-engine/vespa/blob/2300ef0412bdb76f227ae3b497c8ca5e73c5da4e/searchlib/src/vespa/searchlib/fef/rank_program.cpp)
constructs executors from a resolved blueprint graph, binds input/output values,
and binds `MatchData`. It requires the query environment and feature overrides.
Its seeds select which outputs to evaluate; first/second-phase programs and
summary/dump programs are distinct programs, not interchangeable score sources.
[FeatureExecutor::lazy_execute](https://github.com/vespa-engine/vespa/blob/2300ef0412bdb76f227ae3b497c8ca5e73c5da4e/searchlib/src/vespa/searchlib/fef/featureexecutor.h)
accepts a **local** document ID and caches execution for that ID. It does not
retrieve a document, verify its membership, or obtain missing term matches.

[MatchToolsFactory](https://github.com/vespa-engine/vespa/blob/2300ef0412bdb76f227ae3b497c8ca5e73c5da4e/searchcore/src/vespa/searchcore/proton/matching/match_tools.cpp)
builds the query, extracts its terms, prepares query shared state, and creates
the execution plan using the search and attribute contexts. Authoritative
evaluation would need all of these under a compatible read lifetime:

- the selected rank setup, index environment, resolved feature graph and seeds;
- the original query tree and query-term/field handles, statistics and locations;
- rank properties, query feature/tensor values and feature overrides;
- attribute read contexts and query object-store/shared feature state;
- a live document's local ID in that same document database/read context;
- real match data for every dependency requiring it, with the required detail;
- timeout/cancellation state and, for phase-dependent features, phase state.

The Search Kernel boundary in [engine.cc](../core/native/vespa/engine.cc)
(`search_local`) constructs a `SearchRequest`, calls `_db->match`, and copies
the returned hit/features into owned results. It does not retain the evaluator,
query environment, match data or phase state as a generation-bound forced-score
handle. A portable document ID or a copied `SearchReply` cannot reconstruct
these lifetimes or establish that context.

## Why extraction is not proof of forced-score validity

[Matcher::getSummaryFeatures / getRankFeatures](https://github.com/vespa-engine/vespa/blob/2300ef0412bdb76f227ae3b497c8ca5e73c5da4e/searchcore/src/vespa/searchcore/proton/matching/matcher.cpp)
can construct a `DocsumMatcher` from requested local IDs, using a cached search
session or a newly built context.
[DocsumMatcher](https://github.com/vespa-engine/vespa/blob/2300ef0412bdb76f227ae3b497c8ca5e73c5da4e/searchcore/src/vespa/searchcore/proton/matching/docsum_matcher.cpp)
selects summary/dump programs; it delegates to
[ExtractFeatures::get_feature_set](https://github.com/vespa-engine/vespa/blob/2300ef0412bdb76f227ae3b497c8ca5e73c5da4e/searchcore/src/vespa/searchcore/proton/matching/extract_features.cpp),
which calls `search.unpack(docid)` before extracting the feature values.
[SearchIterator::unpack](https://github.com/vespa-engine/vespa/blob/2300ef0412bdb76f227ae3b497c8ca5e73c5da4e/searchlib/src/vespa/searchlib/queryeval/searchiterator.h)
calls `doUnpack` only if `seek(docid)` matches. Passing a nonmatching ID to feature
extraction therefore does not provide the requested document's lexical evidence.

[Bm25Executor](https://github.com/vespa-engine/vespa/blob/2300ef0412bdb76f227ae3b497c8ca5e73c5da4e/searchlib/src/vespa/searchlib/features/bm25_feature.cpp)
obtains term-field handles from the query environment, binds them to match data,
and uses `has_ranking_data(doc_id)` before reading occurrences and field length.
Missing data can yield zero, or degraded scoring when interleaved detail is
missing for a matching term. Such a number does not prove that all terms were
actually examined for an arbitrary document. Distinguish **observed absence of a
term** from **no unpack for the requested document**. BM25's native fallback is
not permission for Search Kernel to synthesize missing data.

[MatchThread](https://github.com/vespa-engine/vespa/blob/2300ef0412bdb76f227ae3b497c8ca5e73c5da4e/searchcore/src/vespa/searchcore/proton/matching/match_thread.cpp)
first ranks documents obtained from the iterator after unpacking. Second phase
scores selected work through
[DocumentScorer](https://github.com/vespa-engine/vespa/blob/2300ef0412bdb76f227ae3b497c8ca5e73c5da4e/searchcore/src/vespa/searchcore/proton/matching/document_scorer.cpp).
[MatchLoopCommunicator](https://github.com/vespa-engine/vespa/blob/2300ef0412bdb76f227ae3b497c8ca5e73c5da4e/searchcore/src/vespa/searchcore/proton/matching/match_loop_communicator.cpp)
also supplies first-phase rank/max shared state. Executing a second-phase
expression for an arbitrary ID cannot prove rerank admission, first-phase rank,
or final score adjustment in the original run.

## Partial feasibility and undefined cases

The native implementations of
[attribute](https://github.com/vespa-engine/vespa/blob/2300ef0412bdb76f227ae3b497c8ca5e73c5da4e/searchlib/src/vespa/searchlib/features/attributefeature.cpp),
[query](https://github.com/vespa-engine/vespa/blob/2300ef0412bdb76f227ae3b497c8ca5e73c5da4e/searchlib/src/vespa/searchlib/features/queryfeature.cpp),
and [constant](https://github.com/vespa-engine/vespa/blob/2300ef0412bdb76f227ae3b497c8ca5e73c5da4e/searchlib/src/vespa/searchlib/features/constant_feature.cpp)
features motivate the bounded possibilities below. These are feature-context
conditions, not advertised backend capabilities.

| Requested evidence | Feasibility at this revision |
| --- | --- |
| Constant or query-only graph | Can be evaluated with the exact configured query/constant context; does not prove document membership. |
| Attribute-only graph | Can be evaluated for a verified live local ID with the appropriate attribute read context. |
| Expression combining those values | Potentially valid only after checking the complete resolved dependency graph, including named functions and object/tensor inputs. |
| Lexical features for a document that still matches but was outside returned hits | Real iterator unpack may make diagnostic reevaluation valid; this is not proof of its original score or stage membership. |
| Lexical features for a document that fails the query iterator | The extraction path above does not supply authoritative per-term evidence; forced matching would require an additional defined hypothesis and native evaluation path. |
| First/second-phase expressions with candidate-set or phase-state dependencies | Undefined without the corresponding state; an independently computable subexpression does not validate the whole phase score. |
| Missing/deleted document, mismatched read context, unavailable inputs, unknown executor dependencies | Unsupported; do not guess defaults or substitute another document/context. |

`FeatureExecutor::isPure()` is a constant-folding promise: output depends only on
inputs. It is not a general counterfactual capability flag. Attribute executors
read by document ID and need not be pure. Conversely, an expression with pure
executors can transitively consume match-dependent inputs. An empty feature
name allowlist, a profile name, or successful numeric evaluation is not a proof
about the resolved graph. Native needed-handle recording in `MatchTools` is
useful for building an unpack plan; it does not establish availability of phase
state or every other contextual dependency.

Any future partial implementation must expose eligibility per requested
feature/phase, reject unknown or unavailable dependencies, and use a separate
diagnostic evaluator with explicit context ownership. It must carry
`execution_kind = COUNTERFACTUAL`, document/query/profile/generation provenance,
and separate evidence identities. Neither its values nor membership claims may
be merged into original-execution evidence. No whole-profile capability should
be inferred from an independently evaluable attribute or constant.

## Evidence limits and verification requirements

The pinned tests
[RankProgramTest](https://github.com/vespa-engine/vespa/blob/2300ef0412bdb76f227ae3b497c8ca5e73c5da4e/searchlib/src/tests/fef/rank_program/rank_program_test.cpp)
(`rank_program_can_calculate_scores_for_multiple_documents`, `const_features`,
`non_const_features`) demonstrate evaluator mechanics. The
[BM25 tests](https://github.com/vespa-engine/vespa/blob/2300ef0412bdb76f227ae3b497c8ca5e73c5da4e/searchlib/src/tests/features/bm25/bm25_test.cpp)
provide explicit testkit match data. Neither establishes an arbitrary-document
operation through Search Kernel's embedded database. These are source evidence,
not real-engine acceptance results for a new capability.

This disposition changes documentation only and enables no execution path.
Enabling a partial evaluator would require real-engine tests for valid independent
graphs and rejection of transitive lexical/phase dependencies, actual unmatched
versus merely unreturned documents, unavailable query tensors, deleted IDs,
generation/read-lifetime changes, and cancellation. Tests must also prove that
normal retrieval/hits are unchanged, counterfactual evidence cannot masquerade
as original evidence, and unsupported sections preserve other W5 diagnostics.
