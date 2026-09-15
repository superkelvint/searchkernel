# `searchkernel-vespa-engine`

This crate is the safe low-level Rust engine API. It owns the wrapper around
the private native ABI, the small schema/config compiler, and the Rust
replacement for the linguistics needed by the embedded engine.

## Start reading here

1. `src/lib.rs` shows engine ownership, native calls, and result conversion.
2. `src/schema.rs` shows schema validation and staged config generation.
3. `src/linguistics.rs` shows tokenization and normalization behavior.
4. `src/linguistics/stemmer.rs` contains the pinned KStem port.
5. Compare behavior with the relevant `reference/vespa` oracle and source
   trace before changing semantics.

## Key concepts

- Vespa owns storage, indexing, matching, ranking, persistence, and resource
  management. This crate adapts those capabilities; it does not reimplement
  them.
- Native handles have exactly one owner and are destroyed by their Rust guard.
- C strings and native buffers are temporary boundary values with explicit
  conversion and cleanup.
- Linguistics must preserve Vespa-compatible token values and source spans.
- Configuration is generated into a temporary directory and installed only
  after successful rendering.

The stable M-layer operations are `Engine::put(Document)`,
`Engine::update(DocumentUpdate)`, `Engine::get`, `Engine::delete`, and
`Engine::search(SearchRequest)`. `put` is complete document replacement;
`update` applies field-level `Assign` or `Clear` mutations. `delete` returns
Vespa's native `RemoveResult::wasFound` value, so `false` means that no
document with the requested ID existed at removal time.
