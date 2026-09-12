# Conformance model

The suite contains four layers of truth:

1. **Normalization goldens** — API declarations/requests -> canonical resolved model.
2. **Validation goldens** — invalid requests -> stable error code + details.
3. **Scripted response goldens** — backend-shaped values -> portable public results.
4. **Stateful semantic scenarios** — management objects evolve through an explicit in-memory state supplied by the case.

No case requires an actual search engine. Search correctness, ranking quality, persistence, and Vespa fidelity belong to the backend oracle suite.

The fixture verifier additionally enforces semantic invariants so copy/template errors cannot pass merely because their hashes match.
