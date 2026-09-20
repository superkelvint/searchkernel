# M12 and Phase 9 intentional limitations

The pinned `NearestNeighborBlueprint::HnswParams` contains these controls:

```text
explore_additional_hits
distance_threshold
global_filter_lower_limit
global_filter_upper_limit
filter_first_upper_limit
filter_first_exploration
exploration_slack
prefetch_tensors
target_hits_max_adjustment_factor
```

Phase 9 exposes `distance_threshold` and `explore_additional_hits` through the
opaque options handle because they have direct portable request semantics and
are required by the acceptance scenarios. The other seven controls remain
deferred: they are execution-policy knobs whose names, defaults, and
interactions are backend/version-sensitive, and the current Portable API has
no frozen semantics for them. They must not be silently guessed or encoded as
positional ABI parameters. A future milestone may add them only with a
source-backed contract and an oracle.

The document path supports the existing portable `array<object>` shape using
`ArrayDataType` plus inline `StructDataType` or `StructReferenceDataType` and
`ValueObject` elements. Nested structs, maps, and heterogeneous object members
remain unsupported because the current native document ABI accepts only
primitive struct members.
