# FlatBuffers wire → IR rules

## Explicit presence wrappers

For `default_search_fields`, request `search_fields`, `projection`, and `highlighting`:

```text
wrapper absent                         -> Absent
wrapper present + inner vector absent  -> PresentEmpty
wrapper present + inner vector []      -> PresentEmpty
wrapper present + values               -> PresentNonEmpty
```

Wrapper presence carries semantics. Inner-vector presence does not.

## Ordinary strings/vectors

FlatBuffers must not invent presence semantics that the portable contract did not declare. Ordinary nullable strings/vectors normalize to their protobuf-equivalent defaults:

```text
null string -> ""
null vector -> []
```

The exception is a field whose presence is explicitly part of the Search Kernel contract, such as optional index name or language.

## Optional scalar wrappers

Missing wrapper means absent. Present wrapper containing zero/false means explicit zero/false. This applies to search-field weight, limits/offset, range inclusivity, missing placement, facet sort, and equivalent presence-sensitive scalars.

## Unions

FlatBuffers union discriminators are decoded exactly once in `*_to_ir`. `NONE` states that correspond to missing required variants become portable validation errors. No generated union discriminator belongs in Search Kernel IR.
