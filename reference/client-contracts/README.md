# Client capability contracts

This directory defines the small amount of human-maintained metadata needed to
turn existing executable tests into a cross-client capability matrix.

The canonical capability IDs and global implementation stages remain in
`docs/capabilities.json`. This registry does **not** duplicate those facts.

## Evidence annotations

Positive client support is declared beside the executable evidence that proves
it:

```text
# SKC: result.highlighting/basic kinds=ergonomic,e2e runtime=grpc
// SKC: search.pagination/basic kinds=ergonomic runtime=shared
```

The fields mean:

- `capability/scenario` — stable contract identity;
- `ergonomic` — the handwritten high-level facade exposes the behavior;
- `conformance` — shared semantic/contract fixtures exercise the behavior;
- `e2e` — the behavior reaches the real product/backend path;
- `runtime` — the environment for that evidence.

Kinds are independent. A low-level transport E2E can prove `e2e` without
proving that the ergonomic facade exposes the feature. Runtime names record the
actual execution path (`grpc`, `grpc-web`, `http`, `managed-local`, etc.);
they do not authorize new transport architectures. An `http` first-party
surface must be explicitly designated HTTP-first by the architecture/SOP and
must use standard FlatBuffers JSON through the official BFBS-backed generic
execute edge.

Surfaces whose production runtime must have executable real-backend coverage
declare `required_e2e_runtimes` in the catalog. Every listed runtime must be
covered by a registered `e2e_lanes` command; an empty or missing lane is a
registry error rather than a vacuous PASS.

Every capability gets a default `basic` scenario. The catalog adds explicit
sub-scenarios when a capability needs finer acceptance tracking. Highlighting,
for example, separately tracks basic highlighting, literal HTML preservation,
and Juniper continuation fragments.

## Human-maintained exceptions

Do not add positive support flags to the catalog. Positive claims come from
test evidence.

The catalog may contain `[[overrides]]` entries only for intentional
`blocked` or `not-applicable` runtime cases. Blocked entries must link a
GitHub issue.

## Generated reports

Run:

```bash
python3 tools/check-client-contracts.py --write
```

This generates:

- `docs/CLIENT_CAPABILITIES.md` — human-readable matrix and evidence links;
- `docs/client-capabilities.json` — machine-readable report.

`./dev check-fast` verifies that annotations are valid and the generated
reports are current.
