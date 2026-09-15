# M9 direct-oracle configuration fixture

All positive M9 scenarios use one verifier-owned low-level schema:

```text
document type: typea

category  String  attribute + summary
qty       Integer attribute + summary
price     Float   attribute + summary
active    Boolean attribute + summary
```

The direct C++ oracle uses one frozen local `DocumentDB` configuration directory for this schema. It must not call the M9 implementation under test to obtain configuration.

The fixture also contains the verifier-only rank profile:

```text
attribute_price
first-phase: attribute(price)
```

`rank-profiles.attribute-price.cfg` in this directory freezes that ranking intent. During authoring, install the semantically equivalent rank-profile config into the direct-oracle fixture and verify the profile by running scenario `attribute-ranking-after-update`.

The base config may be produced once from the already-green M6 compiler or directly from the pinned Vespa config-model, then reviewed and frozen. It is never regenerated during `verify-m9.sh`.
