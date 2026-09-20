# SearchReply oracle specification

`searchreply_oracle` uses the pinned Vespa Proton `DocumentDB` directly. It
loads `scenario.json`, executes the normal request before adding the 200-document
match-phase corpus, then executes the deterministic degraded request.

Each output records whether a usable reply was returned, total and returned hit
counts, the three Vespa coverage counters, the Search Kernel degradation bitset
values defined by the frozen ABI, and copied issue messages. The oracle does
not link to or call Search Kernel production code.

`expected/searchreply.json` is frozen from this direct oracle. The verifier
compares the oracle output and the native C probe output independently against
that file and then compares the two outputs directly.
