# M3 — Linguistics compatibility

M0–M2 are complete and must not regress.

## Goal

Replace the minimum Java linguistics functionality required by the embedded Vespa engine for the supported English text path.

The authoritative comparison is live parity against the pinned Vespa Java implementation:

```text
DOCUMENT
raw UTF-8 text
    -> OpenNlpLinguistics.getTokenizer() [index mode]
    -> indexable leaf tokens
    -> term + source span + position

QUERY
raw UTF-8 text
    -> OpenNlpLinguistics.getStemmer() [query mode]
    -> ordered stem lists
```

The Rust implementation must match that behavior for the frozen M3 corpus.

## Supported scope

Exactly:

- language: English;
- stemming: `NONE`, `DEFAULT`;
- accent removal: false/true;
- normal Vespa lowercasing/normalization implied by the pinned implementation;
- Unicode input;
- index/document token positions and UTF-8 byte spans;
- query stem lists in source order.

Not in M3:

- language detection;
- languages other than English;
- synonyms;
- spelling correction;
- fuzzy/prefix matching;
- query parsing or YQL;
- a general analyzer/plugin framework;
- search, BM25, indexing, or Matcher execution;
- annotation occurrence/length limits (M3 corpus stays below them).

## Normative oracle

The Java oracle MUST instantiate the pinned Vespa `OpenNlpLinguistics` and call the real Vespa interfaces. It MUST NOT contain a tokenizer, normalizer, lowercase implementation, accent remover, or stemmer of its own.

Document-side authority:

```java
new OpenNlpLinguistics()
    .getTokenizer()
    .tokenize(input, Language.ENGLISH, stemMode, removeAccents)
```

The oracle flattens compound tokens the same way Vespa indexing annotations consume them: recurse into non-special compound tokens; otherwise emit special tokens or indexable leaf tokens. For M3 `NONE`/`DEFAULT`, the emitted document term is `Token.getTokenString()`.

Query-side authority:

```java
new OpenNlpLinguistics()
    .getStemmer()
    .stem(input, Language.ENGLISH, stemMode, removeAccents)
```

Every returned `StemList` is preserved, in order, including all stems returned by Vespa.

## Canonical document output

One JSON object per scenario:

```json
{
  "id": "case-id",
  "terms": [
    {
      "original": "Foxes",
      "term": "fox",
      "byte_start": 4,
      "byte_len": 5,
      "position": 0
    }
  ]
}
```

Rules:

- `original` is the exact original input substring;
- `term` is Vespa `Token.getTokenString()`;
- offsets are UTF-8 bytes, not Java UTF-16 units;
- `position` is the zero-based ordinal of emitted indexable occurrences;
- order is source/tokenizer order;
- repeated terms are not deduplicated.

The Java adapter may convert Vespa's Java string offset into UTF-8 byte offsets. That conversion is transport canonicalization, not linguistic behavior.

## Canonical query output

```json
{
  "id": "case-id",
  "terms": [
    {
      "stems": ["fox"],
      "position": 0
    }
  ]
}
```

Every stem returned by a Vespa `StemList` is preserved in list order.

## Rust API frozen by the verifier

The compile fixture and Rust conformance driver freeze this public `searchkernel-vespa` surface:

```rust
Language::English
StemMode::{None, Default}
LinguisticsOptions {
    language,
    stem_mode,
    remove_accents,
}
Linguistics::default()
Linguistics::analyze_document(&str, options) -> DocumentAnalysis
Linguistics::analyze_query(&str, options) -> QueryAnalysis

DocumentAnalysis { terms: Vec<DocumentTerm> }
DocumentTerm {
    original: String,
    term: String,
    byte_start: usize,
    byte_len: usize,
    position: u32,
}

QueryAnalysis { terms: Vec<QueryTerm> }
QueryTerm {
    stems: Vec<String>,
    position: u32,
}
```

The implementation agent may not redesign this API after the harness is frozen.

## Verification

`verify-m3.sh` must prove all of the following:

1. the prior M2 verifier still passes;
2. `cargo fmt --check` passes;
3. clippy with `-D warnings` passes;
4. workspace tests pass;
5. the frozen M3 API contract compiles;
6. the Java oracle builds against `upstream/vespa`;
7. Java document output matches frozen document goldens;
8. Java query output matches frozen query goldens;
9. Rust document output matches the same goldens;
10. Rust query output matches the same goldens;
11. Rust output also matches live Java output directly;
12. the frozen harness checksum is unchanged;
13. production M3 code contains no Java/oracle/golden shortcut.

The live Java comparison is normative. Frozen expected files exist so normal review can see the behavior and so the oracle itself cannot silently drift without detection.

## Forbidden shortcuts

Production code must not:

- spawn Java or the JVM;
- invoke this oracle;
- read `reference/vespa/m3/expected`;
- branch on M3 scenario IDs or fixture strings;
- use a toy ASCII/whitespace tokenizer;
- implement search/BM25;
- depend on `searchkernel-core`;
- substitute a different stemmer merely because common examples happen to agree.

## Definition of done

M3 is complete only when `./reference/vespa/m3/verify-m3.sh` passes from a clean checkout with the pinned `upstream/vespa` tree available to the acceptance environment.
