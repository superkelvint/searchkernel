# M13 grouping/YQL acceptance coverage

This is additive coverage for the existing M13 tensor verifier. The tensor
contract and verifier remain unchanged.

`verify-grouping-yql.sh` runs two real execution paths:

1. the frozen M9 recursive grouping verifier, including its independent pinned
   Vespa C++ oracle and native C ABI probe;
2. the Python V2 showcase, which sends grouping and YQL requests through the
   generated FlatBuffers, daemon transport, adapter, and native Vespa engine.

The coverage manifest records the cases that must be exercised. Grouped summary
hits and continuation round-trips are explicitly marked blocked until the
native result seam supports them; they are not silently treated as passing.

Run it directly with:

```bash
reference/vespa/m13/grouping-yql/verify-grouping-yql.sh
```
