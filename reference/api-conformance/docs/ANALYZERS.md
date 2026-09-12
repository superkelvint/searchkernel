# Analyzer contracts

Analyzer identities are Search Kernel semantics, not aliases for backend defaults.

## `standard/v1`

1. Normalize text with Unicode NFKC.
2. Apply Unicode Default Case Folding.
3. Emit maximal runs of Unicode letters or decimal digits as tokens; all other characters are separators.
4. Do not remove stop words and do not stem.
5. Positions are zero-based and contiguous.

`original` is the source substring, `normalized` is the normalized/case-folded token, and `indexed` equals `normalized`.

## `english/v1`

Apply `standard/v1`, then Porter2 English stemming to each token. Stop words are retained. `normalized` remains the pre-stem token and `indexed` is the Porter2 stem.

## `keyword/v1`

Normalize the complete input using Unicode NFKC plus Unicode Default Case Folding and emit exactly one token. Whitespace and punctuation do not split the token. Empty input emits no token.

Analyzer versions are immutable. A semantic change requires a new version.
