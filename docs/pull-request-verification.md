# Pull request behavioral verification

Substantive pull requests are verified against the exact source head SHA that is
being reviewed. Verification from an older SHA is stale after a rebase,
force-push, conflict resolution, manual branch reconstruction, or any other
head-changing operation.

The `Behavioral verification` workflow is the executable merge-verification
surface. It runs on every pull request without a path filter. Its classifier
selects only the behavioral lanes affected by the changed paths, while the
final `Behavioral verification / Exact-head gate` job is always present.

Every checkout in that workflow explicitly uses
`github.event.pull_request.head.sha`, and every selected lane calls
`.github/ci/exact_head.py assert-head` before testing. A run therefore cannot
reuse behavioral evidence produced from a different checked-out revision.
The workflow concurrency group also cancels an older run when the same pull
request receives a new head.

## Change-aware lanes

The classifier intentionally separates behavior by implementation surface:

| Changed surface | Required behavior |
| --- | --- |
| Rust workspace, API schemas, core crates, or `searchkerneld` | Workspace Rust tests with all targets and no native default features |
| Python client or shared portable/client-contract inputs | Python client unit tests |
| TypeScript client or shared portable/client-contract inputs | TypeScript typecheck and Vitest suite |
| Native adapter, Vespa engine/bindings, IR adapter, or frozen Vespa verifier inputs | The authoritative I-series product gate, `./reference/vespa/verify-through.sh i` |

API schema changes select Rust, Python, and TypeScript because the portable
contract is shared by those surfaces. Documentation-only and CI-only changes do
not run unrelated product suites, but the exact-head helper regression tests
still run in the classifier job.

The native job separates prerequisite bootstrap from product verification. A
failure in `Bootstrap native prerequisites` is an environment/bootstrap failure;
a failure in `Run authoritative native product gate` is a product acceptance
failure. Native verification follows `reference/vespa/README.md`; it is not a
hand-picked substitute for the active product gate.

## Merge-readiness rule

`Behavioral verification / Exact-head gate` is the one status that represents
change-aware behavioral verification. ABI and capability-document workflows are
additional evidence and must not be treated as substitutes for this gate.
Repository protection/rules or the CTO merge process must require the exact-head
gate for substantive pull requests.

The workflow does not replace independent adversarial review. CI proves the
selected executable behavior at one exact revision; review separately checks
backward compatibility, malformed inputs, false greens, cross-language drift,
architectural boundaries, and whether the selected lanes were appropriate.

## Local regression

The CI helper is backend-free and can be checked directly:

```sh
python3 .github/ci/test_exact_head.py
```

The regression creates a temporary Git repository, verifies one commit, changes
the head, and proves that the earlier SHA is rejected. It also freezes the
path-to-lane classification used by the workflow.
