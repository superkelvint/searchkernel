# Error detail contract

`error.code` is stable. `error.details` is also part of conformance: for every error code, all occurrences use the same set of detail keys. Values vary by failure. If a new detail key is required, the semantics version must be reviewed. The verifier enforces shape consistency recursively, including errors nested inside bulk results and stateful-scenario steps.
