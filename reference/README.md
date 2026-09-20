# Reference material and verifiers

The `reference` tree contains frozen contracts, conformance corpora, direct
oracles, and verification scripts. It defines expected behavior independently
of the implementation.

## Start reading here

1. Read `api-conformance/` for the portable API-to-IR semantic corpus.
2. The retired portable client C ABI has no active conformance lane; protocol
   and native-engine contracts are the current boundaries.
3. Read [`vespa/README.md`](vespa/README.md) for the authoritative M0–M13
   verifier map, current integration gate, and specialized active lanes.
4. Read `clients/c2/` for the high-level semantic-core contract.

## Key concepts

- Contracts and goldens are specification inputs, not implementation output.
- Direct C++ or Java behavior is an oracle, not a second production backend.
- Verifiers should exercise the required path and must not be weakened to fit
  an implementation.
- Changes to acceptance artifacts require explicit verifier-authoring intent.
