# M10 direct C++ oracle specification

The oracle is an independent direct client of the pinned Vespa `DocumentDB`.
It must not include or link Search Kernel production source.

For every positive scenario it:

1. starts a real local Proton `DocumentDB` from `fixture/`;
2. inserts the scenario documents with their frozen M3 linguistic annotations;
3. constructs the real low-level Vespa query tree;
4. sets `SearchRequest::ranking`;
5. inserts query features and raw properties into the request's native
   `MapNames::RANK` property map;
6. calls `DocumentDB::match`;
7. records `SearchReply::Hit::metric` and `SearchReply::match_features`;
8. emits exact IEEE-754 bit patterns.

The oracle does **not** call the Rust wrapper, `ConfigCompiler`, the M10 Rust
driver, Java/container search, or a Search Kernel ranking implementation.

`match_features.values` is interpreted as the pinned Vespa contract states:
`names.size() * N` values, one feature row for each returned hit. Any non-double
feature is an explicit M10-v1 oracle failure.
