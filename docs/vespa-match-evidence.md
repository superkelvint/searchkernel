# Per-hit query match evidence at the pinned Proton boundary

Search Kernel cannot copy the original execution's complete term/operator-to-field
match evidence from `DocumentDB::match()` at the current Vespa pin. Proton owns
that evidence inside its matching threads and returns neither the match data nor
an extraction callback. Keep this evidence **unavailable**, rather than deriving
it from document text, highlights, scores, or absent rank features.

This is the source-backed limitation outcome for [#1050](https://github.com/superkelvint/searchkernel/issues/1050).
It does not mean Vespa lacks term-field data, or that configured lexical rank
features cannot expose narrower evidence. It means the existing reply boundary
does not expose a complete snapshot of the original execution. No runtime, ABI,
portable contract, or frontend behavior changes accompany this conclusion.

## Revision and call path

The authority is `vespa_commit` in
[`core/native/vespa/manifest.json`](../core/native/vespa/manifest.json):
`2300ef0412bdb76f227ae3b497c8ca5e73c5da4e`. All upstream links below are
fixed to that revision. Revisit this conclusion when changing the pin.

[`search_local_result` / `search_local`](../core/native/vespa/engine.cc) constructs
a `SearchRequest`, installs the serialized query tree, calls `_db->match()`, and
then copies `SearchReply::match_features` into Search Kernel-owned result rows.
Live-hit compaction keeps those feature rows aligned with the returned hits.
The adapter receives an owned reply; it never receives a `MatchTools` borrow.

The native call continues through:

1. [`DocumentDB::match`](https://github.com/vespa-engine/vespa/blob/2300ef0412bdb76f227ae3b497c8ca5e73c5da4e/searchcore/src/vespa/searchcore/proton/server/documentdb.cpp),
   which takes a shared reference to the ready subdatabase's search view.
2. [`SearchView::match`](https://github.com/vespa-engine/vespa/blob/2300ef0412bdb76f227ae3b497c8ca5e73c5da4e/searchcore/src/vespa/searchcore/proton/server/searchview.cpp)
   and [`MatchView::match`](https://github.com/vespa-engine/vespa/blob/2300ef0412bdb76f227ae3b497c8ca5e73c5da4e/searchcore/src/vespa/searchcore/proton/server/matchview.cpp),
   which select the rank-profile matcher and retain search/attribute contexts,
   the search handler, and the document-meta-store read guard.
3. [`Matcher::match`](https://github.com/vespa-engine/vespa/blob/2300ef0412bdb76f227ae3b497c8ca5e73c5da4e/searchcore/src/vespa/searchcore/proton/matching/matcher.cpp),
   which creates a `MatchToolsFactory` and calls `MatchMaster::match`.
4. [`Query`](https://github.com/vespa-engine/vespa/blob/2300ef0412bdb76f227ae3b497c8ca5e73c5da4e/searchcore/src/vespa/searchcore/proton/matching/query.cpp),
   which builds the Proton query tree, reserves term-field handles in a
   `MatchDataLayout`, creates/optimizes the blueprint tree, fetches postings,
   and creates search iterators using the requested match data.
5. [`MatchThread::run` / `inner_match_loop`](https://github.com/vespa-engine/vespa/blob/2300ef0412bdb76f227ae3b497c8ca5e73c5da4e/searchcore/src/vespa/searchcore/proton/matching/match_thread.cpp),
   which own per-thread tools, seek hits, and unpack match data for ranking
   (or sorting when required). An unranked query need not unpack at all.

## Authoritative structures and their ownership

| Structure | Meaning | Lifetime / limitation |
| --- | --- | --- |
| [`ITermData`](https://github.com/vespa-engine/vespa/blob/2300ef0412bdb76f227ae3b497c8ca5e73c5da4e/searchlib/src/vespa/searchlib/fef/itermdata.h) | Static ranking unit: term/phrase, unique ID, searched fields | Borrowed from the Proton query tree; fields describe where a unit can search, not where this document matched |
| [`ITermFieldData`](https://github.com/vespa-engine/vespa/blob/2300ef0412bdb76f227ae3b497c8ca5e73c5da4e/searchlib/src/vespa/searchlib/fef/itermfielddata.h) | Field ID and handle into match data | Requesting a handle registers the required unpack details; corpus document frequency is not per-hit evidence |
| [`TermFieldMatchData`](https://github.com/vespa-engine/vespa/blob/2300ef0412bdb76f227ae3b497c8ca5e73c5da4e/searchlib/src/vespa/searchlib/fef/termfieldmatchdata.h) | Document ID, positions/occurrences, or operator-specific raw score/subqueries | Mutable scratch for one term-field handle; `reset(docId)` replaces its previous document state |
| [`MatchTools`](https://github.com/vespa-engine/vespa/blob/2300ef0412bdb76f227ae3b497c8ca5e73c5da4e/searchcore/src/vespa/searchcore/proton/matching/match_tools.h) | Owns `MatchData`, rank program, and search iterator | Per-thread/per-phase owner; `match_data()` is a borrow, not an owned per-hit matrix |
| [`SearchReply`](https://github.com/vespa-engine/vespa/blob/2300ef0412bdb76f227ae3b497c8ca5e73c5da4e/searchlib/src/vespa/searchlib/engine/searchreply.h) | Owned hits, coverage, configured match-feature values, properties, issues | Contains no term-field handles, query environment, iterator, match-data snapshot, or callback |

Even while tools are alive, merely scanning all handles is insufficient.
[`MatchTools::setup`](https://github.com/vespa-engine/vespa/blob/2300ef0412bdb76f227ae3b497c8ca5e73c5da4e/searchcore/src/vespa/searchcore/proton/matching/match_tools.cpp)
records rank-program handle requirements before constructing the iterator.
[`HandleRecorder::tag_match_data`](https://github.com/vespa-engine/vespa/blob/2300ef0412bdb76f227ae3b497c8ca5e73c5da4e/searchcore/src/vespa/searchcore/proton/matching/handlerecorder.cpp)
tags other handles as not needed. A missing/stale document ID for a handle that
was not requested is **unknown**, not proof that its term did not match.
`has_data(docId)` checks document identity; `has_ranking_data(docId)` additionally
excludes data hidden from ranking. Neither creates data that was not unpacked.

The ranking term list is also not the portable query-node list.
[`TermDataExtractor`](https://github.com/vespa-engine/vespa/blob/2300ef0412bdb76f227ae3b497c8ca5e73c5da4e/searchcore/src/vespa/searchcore/proton/matching/termdataextractor.cpp)
omits unranked ordinary terms and negative `AndNot` children, handles negative
proximity children separately, and exposes an `Equiv` as a unit. Ranking-term
ordinal zero must not be assumed to mean portable query-node zero.

## Why the later APIs do not recover the original snapshot

The local `MatchTools` in `MatchThread::run` is destroyed on return. Its mutable
data would only describe the most recently unpacked document even if that owner
survived. `MatchMaster` merges results and constructs the reply after the thread
bundle runs; there is no adapter callback between unpacking a hit and advancing
the iterator.

[`make_reply` in `match_master.cpp`](https://github.com/vespa-engine/vespa/blob/2300ef0412bdb76f227ae3b497c8ca5e73c5da4e/searchcore/src/vespa/searchcore/proton/matching/match_master.cpp)
can extract configured match features for the selected hit rows.
[`ExtractFeatures::get_match_features`](https://github.com/vespa-engine/vespa/blob/2300ef0412bdb76f227ae3b497c8ca5e73c5da4e/searchcore/src/vespa/searchcore/proton/matching/extract_features.cpp)
sets `MATCH_FEATURES`, creates fresh tools, unpacks the selected documents, and
copies rank-program seed values. This is engine-generated feature evidence in a
later phase, not a retained first-phase match-data snapshot.

Query caching does not change that distinction. `Matcher::match` can move its
factory and ownership bundle into a
[`SearchSession`](https://github.com/vespa-engine/vespa/blob/2300ef0412bdb76f227ae3b497c8ca5e73c5da4e/searchcore/src/vespa/searchcore/proton/matching/search_session.h).
The session retains the query and contexts, **not** the matching thread's tools
or a per-hit matrix. There is an accessible route through `DocumentDB`'s ready
subdatabase, `SearchView`, `MatchView::getSessionManager()`, and the session
factory; it must not be described as inaccessible. It enables another matching
pass, not access to the original scratch data.

[`DocsumMatcher`](https://github.com/vespa-engine/vespa/blob/2300ef0412bdb76f227ae3b497c8ca5e73c5da4e/searchcore/src/vespa/searchcore/proton/matching/docsum_matcher.cpp)
likewise creates fresh tools for summary/dump features or matching elements.
Matching elements return document/field/element IDs, not term identities.
Rematching is especially significant for stateful operators: the comment on
`MatchTools::allow_reuse_search` explicitly warns that a second-phase iterator
can both miss original hits and match documents not matched in the first phase.
Replaying isolated terms would also discard the original operator context.

## Narrower lexical evidence that is possible

[`FieldTermMatchExecutor`](https://github.com/vespa-engine/vespa/blob/2300ef0412bdb76f227ae3b497c8ca5e73c5da4e/searchlib/src/vespa/searchlib/features/fieldtermmatchfeature.cpp)
reads `TermFieldMatchData` and exports
`fieldTermMatch(indexField, rankingTermOrdinal).occurrences`. A positive value is
authoritative evidence of ranking-visible match data for that unit and document
in the feature evaluation phase. It is not text/highlight inference. This is a
real existing avenue for a restricted lexical diagnostic execution.

Its limits matter: the parameter is a ranking-term ordinal, the blueprint accepts
index fields, and zero is also produced for an illegal/missing handle or hidden
ranking data. It cannot generally distinguish a proven non-match from an
unsupported unit. The pinned
[`field term match tests`](https://github.com/vespa-engine/vespa/blob/2300ef0412bdb76f227ae3b497c8ca5e73c5da4e/searchlib/src/tests/features/prod_features_fieldtermmatch.cpp)
cover an absent query term, positional matches, and a matched unit with no
positions. The last case produces one occurrence: positional counts must not be
interpreted uniformly across operators.

Feature seeds are compiled from index-environment/rank-profile properties in
[`RankSetup::configure` / `compile`](https://github.com/vespa-engine/vespa/blob/2300ef0412bdb76f227ae3b497c8ca5e73c5da4e/searchlib/src/vespa/searchlib/fef/ranksetup.cpp),
not installed dynamically by a search request's rank properties. Preconfiguring
term-slot/field features in every ordinary rank profile would incur extraction
and unpack work on normal searches. A separate diagnostic profile could bound
that work, but would require an explicit ranking-term identity map, truncation
and unsupported states, and separate-execution provenance. It would not repair
the original-execution lifetime limitation described here.

A custom feature executor could in principle copy a richer diagnostic snapshot
while match data is bound. However,
[`Matcher`](https://github.com/vespa-engine/vespa/blob/2300ef0412bdb76f227ae3b497c8ca5e73c5da4e/searchcore/src/vespa/searchcore/proton/matching/matcher.h)
owns a private blueprint factory; its constructor installs the built-in search
features and compiles the rank setup. It offers no adapter registration method
for another executor. This is not an available injection hook on the current
`DocumentDB` path.

Nearest-neighbor/vector, predicate, and geospatial operators need their own
operator-specific evidence and support states. Raw scores, distances, predicate
subqueries, or matching element IDs must not be relabeled as lexical term
occurrences. Composite-unit evidence must not be expanded into claims about
each child without an engine-supported mapping.

## Requirements for revisiting the boundary

An original-execution extraction hook would have to request the relevant handles
before iterator construction and copy evidence while the matching thread owns
the tools, before advancing to another document. It must also handle queries
that currently skip unpacking. The pinned `DocumentDB::match` interface has no
such hook; adding one inside Proton would require changing the read-only upstream
engine. Reimplementing Proton's matching loop in Search Kernel is not an adapter
solution.

A future supported hook or restricted diagnostic rematch must bound returned
hits, units, fields, bytes, and elapsed time; report truncation and unknown states;
and copy into Search Kernel-owned primitive buffers before any native borrow
expires. Normalize evidence in the IR adapter with engine/phase provenance, then
send it through the canonical portable contract. The frontend consumes that
contract and never interprets Vespa handles or rank-feature names.

No extraction hook was installed, so this change adds zero normal-query work by
construction. There is no implemented diagnostic hook whose runtime cost can be
measured; no latency or allocation benchmark is claimed. Any future hook needs
real-engine multi-term/multi-field positive and negative tests, unsupported and
truncated cases, and measurements with diagnostics enabled and disabled.
