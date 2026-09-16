# Authoring the M10 oracle and goldens

The direct oracle is verifier-authoring infrastructure. It is built only after
the M10 API, contract and scenario corpus have been reviewed.

```bash
reference/vespa/m10/oracle/build-cpp-oracle.sh
reference/vespa/m10/oracle/run-cpp-oracle.sh \
  reference/vespa/m10/scenarios/08-multi-feature-inspection.json | python3 -m json.tool
reference/vespa/m10/freeze-goldens.sh
```

Before committing the frozen outputs, review at least these traps manually:

- `bm25-title` and `field-specific-bm25` use different rank profiles/fields;
- changing query feature `boost` changes native score/order;
- typed `query_feature("boost", 2.0)` and raw rank property `boost=2.0`
  produce bit-identical results;
- the arithmetic expression score is not copied from the attribute;
- `native-rank-features` returns all three requested native feature values;
- `multi-feature-inspection` returns exactly the declared match-feature names.

Do not modify the oracle or regenerate expected output after implementation
starts unless M10 is deliberately re-specified and the freeze boundary is
reviewed again.
