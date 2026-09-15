# Ready-to-use milestone prompts

## M0

```text
Implement M0 of Search Kernel.

Acceptance command:

    ./reference/vespa/verify-through.sh m0

The checked-in verifier specification is frozen by its verifier-authoring
commit. Do not modify contracts, scenarios, expected outputs, oracle sources,
or verifier scripts during implementation.

Goal:
Package the pinned embedded Vespa engine as the native SDK defined by
reference/vespa/m0/CONTRACT.md and reference/vespa/NATIVE_ABI_CONTRACT.h.

Do not expose Vespa headers through the public native header.
Do not require the Vespa source checkout or development container at runtime.
Do not change the ABI to make implementation easier.
Keep the adapter minimal.
```

## M1

```text
Implement M1 of Search Kernel.

Acceptance command:

    ./reference/vespa/verify-through.sh m1

M0 is complete and must not regress.
The checked-in verifier specification is frozen by its verifier-authoring
commit. Do not modify contracts, scenarios, expected outputs, oracle sources,
or verifier scripts during implementation.

Implement the exact Rust API frozen in reference/vespa/API_CONTRACT.md.
Safely own the M0 native engine from Rust.

Do not create a fake Rust engine.
Do not bypass searchkernel-vespa-bindings.
Do not change the API contract.
Do not expose raw pointers or Vespa C++ types publicly.
```

## M2

```text
Implement M2 of Search Kernel.

Acceptance command:

    ./reference/vespa/verify-through.sh m2

M0-M1 are complete and must not regress.
The checked-in verifier specification is frozen by its verifier-authoring
commit. Do not modify contracts, scenarios, expected outputs, oracle sources,
or verifier scripts during implementation.

Implement the exact M2 Rust API frozen in reference/vespa/API_CONTRACT.md.
Reproduce the behavior in reference/vespa/m2/CONTRACT.md through the real
native Vespa engine.

Do not implement matching or document state in Rust.
Do not use a mock/fake backend in the acceptance path.
Do not hard-code golden IDs or values in production code.
Do not invoke the old oracle executable as the implementation.
Do not change or regenerate expected output.
Keep the change minimal.
```
