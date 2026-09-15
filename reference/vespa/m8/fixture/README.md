# M8 direct-oracle configuration fixtures

The independent C++ oracle must **not** consume configuration produced on the fly by the M8 implementation under test.

Freeze two verifier-owned local `DocumentDB` configuration directories during verifier authoring:

```text
fixture/attribute/
  document type: typea
  qty       Integer attribute + summary
  priority  Integer attribute + summary

fixture/text/
  document type: typea
  title     Text index + summary, NOT attribute
  rank profiles: default, bm25(first-phase = bm25(title))
```

Scenario mapping:

```text
01-05, 08 -> attribute
06-07     -> text
```

The fixture configuration may be produced once from the already-green M6 compiler or directly from the pinned Vespa config-model, then manually reviewed and frozen as verifier input. It is never regenerated during `verify-m8.sh`.

Required files are the same low-level config set consumed by the current embedded `DocumentDB` bootstrap, including at minimum:

```text
searchkernel-schema.json
indexschema.cfg
attributes.cfg
summary.cfg
rank-profiles.cfg
juniperrc.cfg
```

and any additional pinned-engine files required by the local bootstrap.

Record fixture provenance when authoring `m8_local_oracle.cpp`. The C++ oracle must choose the fixture from the frozen scenario family, not from production Search Kernel output.
