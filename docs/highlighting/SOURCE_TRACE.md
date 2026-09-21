# Vespa highlighting source trace

This implementation was checked against pinned Vespa revision
`2300ef0412bdb76f227ae3b497c8ca5e73c5da4e` (`v8.751.13-230-g2300ef0412b`).

The relevant upstream path is the ordinary document-summary dynamic-teaser
path, not a Search Kernel text post-processor:

- `searchsummary/src/vespa/searchsummary/docsummary/dynamicteaserdfw.cpp`
  reads the stored summary field, builds a `juniper::Query` from the
  `DocsumRequest` serialized query tree, calls `juniper::Analyse`, and returns
  `juniper::GetTeaser` output.
- `searchsummary/src/vespa/searchsummary/docsummary/juniper_query_adapter.cpp`
  converts the serialized query tree into the Juniper query/highlight terms.
- `searchsummary/src/vespa/searchsummary/docsummary/annotation_converter.cpp`
  preserves Vespa linguistic annotations as U+FFF9/U+FFFA/U+FFFB
  interlinear-annotation sequences.
- `searchsummary/src/vespa/searchsummary/docsummary/juniperproperties.cpp`
  configures dynamic-teaser highlight markers as the control byte `0x1f`.
- `searchsummary/src/vespa/juniper/sumdesc.cpp` and
  `searchsummary/src/vespa/juniper/config.cpp` implement teaser selection and
  marker emission. Juniper uses `0x1e` between disjoint selected teaser
  regions; Search Kernel maps those boundaries to separate highlight
  fragments. `searchsummary/src/vespa/juniper/matchobject.cpp` handles matching
  through the linguistic annotations.

Search Kernel generates one hidden `dynamicteaser` summary field per eligible
stored/indexed text field. The native adapter asks `DocumentDB::getDocsums()`
for those fields, sets the serialized query tree on the docsum request, and
decodes only Vespa's returned markers/annotations into the canonical
`Hit.highlights` spans. Ordinary projected fields remain ordinary `Hit.fields`.

The native unit test covers the marker/annotation decoder independently of the
engine. The real-engine test covers query-tree propagation, `getDocsums()`,
multiple fields, projection interaction, and Vespa stemming behavior.
