# C2 semantic-core client

This directory contains the standalone contract and verifier for the semantic
core client. It exercises the portable FlatBuffers API without Vespa, Docker,
the native SDK, or a search backend.

## Start reading here

1. Read `CONTRACT.md` for required behavior and explicit non-goals.
2. Read `api-contract/` for the compile-time client surface.
3. Read `scenarios/` and `expected/` for concrete semantic cases.
4. Run `./verify-c2.sh` to see the complete check.

## Key concepts

- API-to-IR conversion owns defaults, presence semantics, validation, and
  stable portable errors.
- `Describe` and `InspectQuery` are semantic operations and do not execute
  searches.
- The backend interface receives already-owned IR and must not invent defaults.
- Scenario and expected files are frozen acceptance artifacts.
