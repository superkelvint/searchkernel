# M10 direct-oracle ranking fixture

All M10 positive scenarios use one verifier-owned local DocumentDB schema:

```text
document type: typea

title       Text     index + summary
body        Text     index + summary
popularity  Integer  attribute + summary
```

The direct C++ oracle must use the frozen files in this directory and must not call the production `ConfigCompiler`.

The fixture freezes these profiles:

```text
bm25_title
  first-phase: bm25(title)
  match-features: bm25(title)

bm25_body
  first-phase: bm25(body)
  match-features: bm25(body)

popularity
  first-phase: attribute(popularity)
  match-features: attribute(popularity)

query_boost
  first-phase: query(boost) * attribute(popularity)
  match-features: query(boost), attribute(popularity)

expression
  first-phase: 2 * attribute(popularity) + 3
  match-features: attribute(popularity)

native_title
  first-phase: nativeRank(title)
  match-features: nativeRank(title), nativeFieldMatch(title), nativeProximity(title)

inspect_all
  first-phase: bm25(title) + query(boost) * attribute(popularity)
  match-features: bm25(title), query(boost), attribute(popularity)
```

Before freezing goldens, regenerate or independently verify this config against the pinned Vespa revision. Once reviewed, the files become immutable verifier inputs.
