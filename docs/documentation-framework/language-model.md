# Language Model

Programming language is a **presentation dimension**, not a documentation hierarchy.

Search Kernel has one developer documentation product.

## Prohibited structure

Do not create parallel trees such as:

```text
/docs/python/filtering
/docs/typescript/filtering
/docs/php/filtering
/docs/rust/filtering
```

The canonical subject is one page, for example:

```text
Filter search results
```

## Global language preference

The documentation UI should expose one persistent language preference, for example:

```text
Language: TypeScript ▾
```

Changing the language may change:

- executable code examples;
- API signatures;
- package installation commands;
- host-language terminology;
- genuine language-specific warnings or callouts.

Changing the language must not change:

- the conceptual organization;
- Search Kernel semantics;
- the page hierarchy;
- the canonical explanation of a capability.

Shared prose is the default.

## Genuine language differences

Language-specific prose is appropriate when the host language materially changes developer behavior.

Examples include:

- JavaScript safe-integer and `bigint` behavior;
- Python lifecycle idioms;
- PHP runtime or packaging constraints;
- language-specific async behavior;
- language-native path, duration, or container types.

Render these as conditional language callouts rather than cloning the page.

## Missing capability support

Never silently fall back to another language.

If the selected language does not expose an ergonomic capability, the documentation should render an explicit unavailable state and may link to a low-level escape hatch if one genuinely exists.

Availability must come from canonical capability evidence, not manually maintained prose.

## Client implementation documentation

The global language selector is for users consuming Search Kernel.

Documentation about implementing or maintaining a language facade belongs under **Contribute to SearchKernel**. It may contain language-specific implementation details, but it still does not create a separate end-user documentation hierarchy.

For executable example IDs, extraction, verification, generated API reference, and capability-aware rendering, see the unified developer-documentation implementation plan.
